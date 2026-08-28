//! GUI handlers reading and writing the operator-facing preferences: start at
//! login, and automatic updates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::gui::ipc::IpcReplyPayload;
use crate::gui::{GuiApp, emit};
use crate::{config, install, update};

pub(crate) fn on_settings_read(app: &GuiApp, reply_to: ReplyId) {
    if let Some(id) = reply_to {
        let payload = IpcReplyPayload::ok(current());
        emit::send_reply_payload(app, id, &payload);
    }
}

pub(crate) fn on_settings_write(app: &mut GuiApp, key: &str, value: &Value, reply_to: ReplyId) {
    let result = write_setting(app, key, value);
    app.refresh_ui();
    let Some(id) = reply_to else {
        return;
    };
    let payload = match result {
        Ok(()) => IpcReplyPayload::ok(current()),
        Err(e) => IpcReplyPayload::err(crate::gui::ipc::BridgeError::internal(e.to_string())),
    };
    emit::send_reply_payload(app, id, &payload);
}

pub(crate) fn on_autostart_toggled(app: &mut GuiApp) {
    let status = install::gui_autostart_status();
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

fn write_setting(app: &GuiApp, key: &str, value: &Value) -> Result<(), GuiError> {
    // Why: every write rewrites the config file, so a file we could not parse
    // must stop the write rather than be replaced by defaults. `Config::load`
    // returns defaults for a malformed file by design and cannot be used here.
    if !matches!(key, "autostart")
        && let Err(e) = config::read()
    {
        return Err(GuiError::Io(std::io::Error::other(format!(
            "refusing to save: {e}"
        ))));
    }
    match key {
        "autostart" => set_autostart(app, as_bool(key, value)?),
        "update_automatic" => config::set_update_automatic(as_bool(key, value)?)
            .map_err(|e| GuiError::Io(std::io::Error::other(e.to_string()))),
        "session_enabled" => config::set_session_enabled(as_bool(key, value)?)
            .map_err(|e| GuiError::Io(std::io::Error::other(e.to_string()))),
        other => Err(GuiError::Io(std::io::Error::other(format!(
            "unknown setting: {other}"
        )))),
    }
}

fn as_bool(key: &str, value: &Value) -> Result<bool, GuiError> {
    value.as_bool().ok_or_else(|| {
        GuiError::Io(std::io::Error::other(format!(
            "setting {key} expects a boolean"
        )))
    })
}

fn set_autostart(app: &GuiApp, enabled: bool) -> Result<(), GuiError> {
    if enabled {
        let binary = update::installed_path()
            .map_err(|e| GuiError::Io(std::io::Error::other(e.to_string())))?;
        let lines = install::apply_gui_autostart(&binary)
            .map_err(|e| GuiError::Io(std::io::Error::other(e.to_string())))?;
        for line in lines {
            app.append_log(line);
        }
    } else {
        app.append_log(match install::remove_gui_autostart() {
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

fn current() -> Value {
    let malformed = config::read().err().map(|e| e.to_string());
    let cfg = config::load();
    let claude = cfg.claude.as_ref();
    json!({
        "autostart": install::gui_autostart_status(),
        "update_automatic": update::automatic_enabled(),
        "gateway_url": config::gateway_url_or_default(&cfg).as_str(),
        "session_enabled": cfg.session.and_then(|s| s.enabled).unwrap_or(false),
        "auth_scheme": claude.and_then(|c| c.auth_scheme.clone()),
        "models": claude.and_then(|c| c.models.clone()),
        "cert_keystore_ref": cfg.cert_keystore_ref().map(crate::ids::KeystoreRef::as_str),
        "pinned_pubkey": pinned_pubkey_value(),
        "config_file": config::config_path().map(|p| p.display().to_string()),
        "config_malformed": malformed,
        "schedule": schedule_value(),
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

fn schedule_value() -> Value {
    json!({
        "state": install::schedule_status(),
        "label": install::schedule_label(),
    })
}
