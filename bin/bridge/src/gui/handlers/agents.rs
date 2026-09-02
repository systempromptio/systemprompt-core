//! GUI handlers for host-app install/uninstall/open actions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::{GuiApp, emit, window};
use crate::ids::HostId;
use crate::integration::find_host_by_id;
use crate::integration::host_app::ProfileRemoval;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

pub(crate) fn on_uninstall(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    // Why: a sync-only agent (`claude-code`) is listed in the Agents tab but
    // implements no `HostApp`, so the lookup below cannot find it. Nothing of
    // it lives on this computer, so this is a no-op — answering "unknown host"
    // would name the one agent most readers are actually running.
    if let Some(value) = crate::gui::sync_only::noop_reply(app, host_id.as_str(), "remove") {
        finish(app, Ok(value), reply_to);
        return;
    }
    let result = find_host_by_id(host_id.as_str()).map_or_else(
        || {
            app.append_log_error(format!("uninstall: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| match host.remove_profile() {
            Ok(ProfileRemoval::Removed { path }) => {
                app.append_log(format!(
                    "[{host_id}] removed from {}",
                    path.as_deref().unwrap_or("its configuration")
                ));
                Ok(json!({ "removed": true, "path": path }))
            },
            Ok(ProfileRemoval::NothingToRemove) => {
                app.append_log(format!("[{host_id}] had no settings left to remove"));
                Ok(json!({ "removed": false }))
            },
            Ok(ProfileRemoval::ManualStepRequired { instruction }) => {
                app.append_log_warn(format!(
                    "[{host_id}] removal needs a manual step: {instruction}"
                ));
                Ok(json!({ "removed": false, "instruction": instruction }))
            },
            Err(e) => {
                app.append_log_error(format!("[{host_id}] removal failed: {e}"));
                Err(BridgeError::new(
                    ErrorScope::Host,
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        ErrorCode::Unauthorized
                    } else {
                        ErrorCode::Internal
                    },
                    format!("could not remove {}: {e}", host.display_name()),
                ))
            },
        },
    );
    // Why: the removal changed what is on disk, so the row must not keep
    // reporting the profile it no longer has.
    if result.is_ok() {
        app.proxy.send_event(crate::gui::events::UiEvent::Host(
            crate::gui::hosts::events::HostUiEvent::ProbeRequested {
                host_id: host_id.clone(),
                cause: crate::gui::hosts::events::ProbeCause::Manual,
                reply_to: None,
            },
        ));
    }
    finish(app, result, reply_to);
}

pub(crate) fn on_open_config(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    // Why: sync-only agents have no local config file; see `on_uninstall`.
    if let Some(value) =
        crate::gui::sync_only::noop_reply(app, host_id.as_str(), "show config file")
    {
        finish(app, Ok(value), reply_to);
        return;
    }
    let result = find_host_by_id(host_id.as_str()).map_or_else(
        || {
            app.append_log_error(format!("open-config: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| {
            let snapshot = host.probe(&app.probe_env());
            snapshot.profile_source.as_ref().map_or_else(
                || {
                    let msg = format!(
                        "open-config: no resolved config path for {}",
                        host.display_name()
                    );
                    app.append_log_error(&msg);
                    Err(BridgeError::new(ErrorScope::Host, ErrorCode::NotFound, msg))
                },
                |path| {
                    window::open_path(Path::new(path));
                    app.append_log(format!(
                        "opened config for {} at {path}",
                        host.display_name()
                    ));
                    Ok(json!({ "path": path }))
                },
            )
        },
    );
    finish(app, result, reply_to);
}

pub(crate) fn on_open(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    // Why: sync-only agents have nothing local to launch; see `on_uninstall`.
    if let Some(value) = crate::gui::sync_only::noop_reply(app, host_id.as_str(), "open") {
        finish(app, Ok(value), reply_to);
        return;
    }
    let result = find_host_by_id(host_id.as_str()).map_or_else(
        || {
            app.append_log_error(format!("open: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| match host.open() {
            Ok(()) => {
                app.append_log(format!("opened host {}", host.display_name()));
                Ok(json!({}))
            },
            Err(err) => {
                let msg = format!("open host {} failed: {err}", host.display_name());
                app.append_log_error(&msg);
                Err(BridgeError::new(ErrorScope::Host, ErrorCode::Internal, msg))
            },
        },
    );
    finish(app, result, reply_to);
}

pub(crate) fn on_setup_complete(app: &mut GuiApp) {
    if app.state.first_run_active() {
        return;
    }
    app.state.set_agents_onboarded(true);
    // Why: the snapshot flag alone does not survive the process, and the next
    // launch re-derives "needs setup" from whether any host still reports an
    // installed profile -- so a user who finished setup is put back through it
    // after uninstalling the last profile.
    crate::gui::onboarding::mark_complete();
    app.append_log("setup marked complete by user");
    emit::emit_state(app);
}

fn finish(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            emit::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
