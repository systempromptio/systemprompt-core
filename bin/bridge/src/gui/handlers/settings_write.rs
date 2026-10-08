//! GUI handlers for the settings snapshot and the tray's start-at-login toggle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::IpcReplyPayload;
use crate::{config, install, update};

pub(crate) fn on_settings_read(app: &GuiApp, reply_to: ReplyId) {
    if let Some(id) = reply_to {
        let payload = match current(&app.ctx.schedule).map(serde_json::to_value) {
            Ok(Ok(value)) => IpcReplyPayload::ok(value),
            Ok(Err(e)) => IpcReplyPayload::err(crate::wire::ipc::BridgeError::internal(format!(
                "settings encode failed: {e}"
            ))),
            Err(e) => IpcReplyPayload::err(crate::wire::ipc::BridgeError::internal(e.to_string())),
        };
        emit::send_reply_payload(app, id, &payload);
    }
}

pub(crate) fn on_autostart_toggled(app: &mut GuiApp) {
    let status = install::gui_autostart_status(&app.ctx.schedule);
    if status == install::ScheduleStatus::Unknown {
        app.append_log_warn("start at login: could not ask the scheduler whether it is registered");
        return;
    }
    let enabled = status == install::ScheduleStatus::Installed;
    if let Err(e) = set_autostart(app, !enabled) {
        app.append_log_warn(format!("start at login: {e}"));
    }
    app.refresh_ui();
    emit::emit_state(app);
}

fn set_autostart(app: &GuiApp, enabled: bool) -> Result<(), GuiError> {
    if enabled {
        let binary = update::installed_path()?;
        let lines = install::apply_gui_autostart(&app.ctx.schedule, &binary)?;
        for line in lines {
            app.append_log(line);
        }
    } else {
        app.append_log(match install::remove_gui_autostart(&app.ctx.schedule) {
            install::ScheduleRemoval::Removed(label) => {
                format!("start at login disabled: {label}")
            },
            install::ScheduleRemoval::NotInstalled(label) => {
                format!("start at login was not registered: {label}")
            },
            install::ScheduleRemoval::Failed(e) => {
                return Err(GuiError::Io(std::io::Error::other(e)));
            },
        });
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct SettingsSnapshot {
    gateway_url: String,
    auth_scheme: Option<String>,
    models: Option<Vec<String>>,
    pinned_pubkey: Option<PinnedPubkeyPayload>,
    config_file: Option<String>,
    config_malformed: Option<String>,
    schedule: SchedulePayload,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PinnedPubkeyPayload {
    Pinned {
        value: String,
        source: &'static str,
    },
    StaleForGateway {
        trust_required: bool,
        pinned_for: config::GatewayIdentity,
        gateway: config::GatewayIdentity,
    },
}

#[derive(Debug, Serialize)]
struct SchedulePayload {
    verdict: crate::verdict::Verdict<install::ScheduleStatus>,
    label: &'static str,
}

fn current(
    schedule: &crate::schedule::status::ScheduleStatusCache,
) -> Result<SettingsSnapshot, GuiError> {
    let malformed = config::read().err().map(|e| e.to_string());
    let cfg = config::load()?;
    let claude = cfg.claude.as_ref();
    Ok(SettingsSnapshot {
        gateway_url: config::gateway_url_or_default(&cfg).as_str().to_owned(),
        auth_scheme: claude.and_then(|c| c.auth_scheme.clone()),
        models: claude.and_then(|c| c.models.clone()),
        pinned_pubkey: pinned_pubkey_payload()?,
        config_file: config::config_path().map(|p| p.display().to_string()),
        config_malformed: malformed,
        schedule: schedule_payload(schedule),
    })
}

fn pinned_pubkey_payload() -> Result<Option<PinnedPubkeyPayload>, GuiError> {
    Ok(match config::pinned_pubkey_state()? {
        config::PinnedPubkeyState::Pinned { key, source } => Some(PinnedPubkeyPayload::Pinned {
            value: key.as_str().to_owned(),
            source: source.label(),
        }),
        config::PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => Some(PinnedPubkeyPayload::StaleForGateway {
            trust_required: true,
            pinned_for,
            gateway: current,
        }),
        config::PinnedPubkeyState::Unpinned => None,
    })
}

fn schedule_payload(schedule: &crate::schedule::status::ScheduleStatusCache) -> SchedulePayload {
    SchedulePayload {
        verdict: install::schedule_status(schedule).verdict(),
        label: install::schedule_label(),
    }
}
