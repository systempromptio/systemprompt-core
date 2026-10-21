//! Agent command dispatch: lifecycle actions on an installed agent and the
//! OS surfaces that manage its profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::args::HostIdArgs;
use super::{CommandOutcome, parse, send};

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn agent_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "agent.uninstall" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentUninstall {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.openConfig" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpenConfig {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.enrol" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentEnrol {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.restart" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::Host(HostUiEvent::RestartRequested {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    }),
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.openDeviceManagement" => CommandOutcome::Sync(open_device_management()),
        "agent.revealProfile" => CommandOutcome::Sync(
            parse::<HostIdArgs>(args).and_then(|a| reveal_profile(app, a.host_id)),
        ),
        "agent.open" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpen {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        _ => return None,
    })
}

#[cfg(not(target_os = "macos"))]
// JSON: webview IPC reply — never produced off macOS.
fn open_device_management() -> Result<Value, BridgeError> {
    Err(BridgeError::invalid_args(
        "Device Management is a macOS settings pane",
    ))
}

#[cfg(target_os = "macos")]
// JSON: webview IPC reply — an empty object on success.
fn open_device_management() -> Result<Value, BridgeError> {
    opener::open(crate::install::mdm::DEVICE_MANAGEMENT_URL)
        .map(|()| serde_json::json!({}))
        .map_err(|e| {
            BridgeError::from_error_in(
                ErrorScope::Host,
                ErrorCode::Internal,
                "open Device Management",
                &e,
            )
        })
}

// Why: an expired macOS profile can still be added by hand with the + in
// Device Management, which needs the file the bridge last generated; Finder's
// reveal (`open -R`) selects it there.
// JSON: webview IPC reply — an empty object on success.
fn reveal_profile(
    app: &GuiApp,
    host_id: systemprompt_models::bridge::host::HostKind,
) -> Result<Value, BridgeError> {
    let path = app
        .state
        .snapshot()
        .hosts
        .get(host_id)
        .and_then(|s| s.last_generated_profile.as_ref().map(|p| p.path.clone()))
        .ok_or_else(|| {
            BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                "no profile has been generated for this agent yet",
            )
        })?;
    let revealed = if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/open")
            .args(["-R", &path])
            .status()
            .is_ok_and(|s| s.success())
    } else {
        std::path::Path::new(&path)
            .parent()
            .is_some_and(|dir| opener::open(dir).is_ok())
    };
    if revealed {
        Ok(serde_json::json!({ "path": path }))
    } else {
        Err(BridgeError::new(
            ErrorScope::Host,
            ErrorCode::Internal,
            format!("could not show {path}"),
        ))
    }
}
