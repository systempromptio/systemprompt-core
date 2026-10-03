//! Meta, gateway, auth and sync command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, server_json};
use crate::ids::McpServerId;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::args::{
    CancelArgs, GatewaySetArgs, LoginArgs, McpProbeArgs, OpenExternalUrlArgs, RecentArgs,
    SessionLoginArgs,
};
use super::{CommandOutcome, parse, send};

const DEFAULT_RECENT_LIMIT: usize = 500;
const MAX_RECENT_LIMIT: usize = 2000;

// JSON: webview IPC args — decoded per command with `parse::<T>`.
fn recent_limit(args: &Value) -> usize {
    parse::<RecentArgs>(args.clone())
        .ok()
        .and_then(|a| a.limit)
        .unwrap_or(DEFAULT_RECENT_LIMIT)
        .min(MAX_RECENT_LIMIT)
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn meta_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: &Value,
    _reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "state.snapshot" => CommandOutcome::Sync(state_snapshot(app)),
        "marketplace.list" => CommandOutcome::Sync(marketplace_listing(app).and_then(|listing| {
            serde_json::to_value(listing).map_err(|e| {
                BridgeError::from_error(ErrorScope::Marketplace, ErrorCode::Internal, &e)
            })
        })),
        "activity.recent" => CommandOutcome::Sync(Ok(json!({
            "entries": app.ctx.activity.snapshot_recent(recent_limit(args)),
        }))),
        "setup.complete" => {
            send(app, UiEvent::SetupComplete);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openConfigFolder" => {
            send(app, UiEvent::OpenConfigFolder);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openExternalUrl" => open_external_url(args.clone()),
        "application.removalGuidance" => super::removal::guidance(),
        "application.reveal" => {
            send(app, UiEvent::RevealApplication);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "quit" => {
            send(app, UiEvent::Quit);
            CommandOutcome::Sync(Ok(json!({})))
        },
        _ => return None,
    })
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn gateway_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "gateway.set" => match parse::<GatewaySetArgs>(args) {
            Ok(a) if a.url.trim().is_empty() => CommandOutcome::Sync(Err(BridgeError::new(
                ErrorScope::Gateway,
                ErrorCode::InvalidArgs,
                "gateway url is empty",
            ))),
            Ok(a) => {
                send(
                    app,
                    UiEvent::SetGatewayRequested {
                        url: a.url,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "gateway.probe" => {
            send(app, UiEvent::GatewayProbeRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "mcp.auth.probe" => {
            let server_id = parse::<McpProbeArgs>(args)
                .inspect_err(|e| tracing::warn!(error = ?e, "malformed mcp.auth.probe args"))
                .ok()
                .and_then(|a| a.server_id)
                .and_then(|s| McpServerId::try_new(s).ok());
            send(
                app,
                UiEvent::McpAuthProbeRequested {
                    server_id,
                    reply_to: reply_id,
                },
            );
            CommandOutcome::Async
        },
        _ => return None,
    })
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn auth_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "login" => match parse::<LoginArgs>(args) {
            Ok(a) if a.token.expose().trim().is_empty() => CommandOutcome::Sync(Err(
                BridgeError::new(ErrorScope::Identity, ErrorCode::InvalidArgs, "PAT is empty"),
            )),
            Ok(a) => {
                send(
                    app,
                    UiEvent::LoginRequested {
                        token: a.token,
                        gateway: a.gateway,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "session.login" => match parse::<SessionLoginArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::SessionLoginRequested {
                        gateway: a.gateway,
                        keep_signed_in: a.keep_signed_in,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "logout" => {
            send(app, UiEvent::LogoutRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "system.purge" => {
            send(app, UiEvent::PurgeRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "system.disconnect" => {
            send(app, UiEvent::DisconnectRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "device.action.dismiss" => {
            app.state.set_pending_device_action(None);
            send(app, UiEvent::StateRefreshed);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "profile.fetch" => {
            send(app, UiEvent::ProfileFetchRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        _ => return None,
    })
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn sync_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "sync" => {
            send(app, UiEvent::SyncRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "validate" => {
            send(app, UiEvent::ValidateRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.check" => {
            send(app, UiEvent::UpdateCheckRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.install" => {
            send(app, UiEvent::UpdateInstallRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.restart" => {
            send(app, UiEvent::UpdateRestartRequested);
            CommandOutcome::Sync(Ok(Value::Null))
        },
        "settings.get" => {
            send(app, UiEvent::SettingsReadRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "cancel" => match parse::<CancelArgs>(args) {
            Ok(a) => match cancel_scope(a.scope.as_deref()) {
                Ok(scope) => {
                    send(
                        app,
                        UiEvent::CancelInFlight {
                            scope,
                            reply_to: reply_id,
                        },
                    );
                    CommandOutcome::Async
                },
                Err(e) => CommandOutcome::Sync(Err(e)),
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        _ => return None,
    })
}

fn cancel_scope(label: Option<&str>) -> Result<Option<CancelScope>, BridgeError> {
    Ok(match label {
        None | Some("all") => None,
        Some("sync") => Some(CancelScope::Sync),
        Some("login") => Some(CancelScope::Login),
        Some("gateway" | "gateway-probe") => Some(CancelScope::GatewayProbe),
        Some(other) => {
            return Err(BridgeError::invalid_args(format!(
                "unknown cancel scope: {other}"
            )));
        },
    })
}

// JSON: webview IPC args — decoded per command with `parse::<T>`.
fn open_external_url(args: Value) -> CommandOutcome {
    match parse::<OpenExternalUrlArgs>(args) {
        Ok(a) => match crate::wire::external_url::ExternalUrl::parse(&a.url) {
            Err(rejected) => {
                CommandOutcome::Sync(Err(BridgeError::invalid_args(rejected.to_string())))
            },
            Ok(url) => match opener::open(url.as_str()) {
                Ok(()) => CommandOutcome::Sync(Ok(json!({}))),
                Err(e) => CommandOutcome::Sync(Err(BridgeError::new(
                    ErrorScope::Internal,
                    ErrorCode::Internal,
                    format!("open url failed: {e}"),
                ))),
            },
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

// JSON: webview IPC reply — each command's typed result serialized at the call
// site.
fn state_snapshot(app: &GuiApp) -> Result<Value, BridgeError> {
    let snap = app.state.snapshot();
    serde_json::to_value(server_json::state_payload(&snap, &app.ctx.proxy)).map_err(|e| {
        BridgeError::from_error_in(
            ErrorScope::Internal,
            ErrorCode::Internal,
            "state encode failed",
            &e,
        )
    })
}

fn marketplace_listing(
    app: &GuiApp,
) -> Result<crate::gui::server_marketplace::MarketplaceListing, BridgeError> {
    let snap = app.state.snapshot();
    crate::gui::server_marketplace::build_listing(
        app.ctx.proxy.loopback(),
        &app.ctx.mcp_registry(),
        &snap.mcp_auth,
    )
    .map_err(|e| BridgeError::from_error(ErrorScope::Marketplace, ErrorCode::Internal, &e))
}
