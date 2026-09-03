//! GUI handlers for the settings snapshot and the tray's start-at-login toggle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::IpcReplyPayload;
use crate::{config, install, update};

pub(crate) fn on_settings_read(app: &GuiApp, reply_to: ReplyId) {
    if let Some(id) = reply_to {
        let payload = IpcReplyPayload::ok(current(&app.ctx.schedule));
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

fn current(schedule: &crate::schedule::status::ScheduleStatusCache) -> Value {
    let malformed = config::read().err().map(|e| e.to_string());
    let cfg = config::load();
    let claude = cfg.claude.as_ref();
    json!({
        "gateway_url": config::gateway_url_or_default(&cfg).as_str(),
        "auth_scheme": claude.and_then(|c| c.auth_scheme.clone()),
        "models": claude.and_then(|c| c.models.clone()),
        "cert_keystore_ref": cfg.cert_keystore_ref().map(crate::ids::KeystoreRef::as_str),
        "pinned_pubkey": pinned_pubkey_value(),
        "config_file": config::config_path().map(|p| p.display().to_string()),
        "config_malformed": malformed,
        "schedule": schedule_value(schedule),
    })
}

// Why: a managed policy silently replaces an operator-set `sync.pinned_pubkey`
// (see `Config::with_policy_overrides`), so the provenance travels with the
// value — without it a supply-chain control can be swapped and the operator has
// no way to see it happened or know the field is no longer theirs to edit.
fn pinned_pubkey_value() -> Value {
    let Some(effective) = config::pinned_pubkey() else {
        return Value::Null;
    };
    let source = if config::policy_pubkey().is_some_and(|p| p.as_str() == effective.as_str()) {
        "policy"
    } else {
        "operator"
    };
    json!({ "value": effective.as_str(), "source": source })
}

fn schedule_value(schedule: &crate::schedule::status::ScheduleStatusCache) -> Value {
    json!({
        "verdict": install::schedule_status(schedule).verdict(),
        "label": install::schedule_label(),
    })
}
