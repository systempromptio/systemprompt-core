//! GUI handlers for host-app install/uninstall/open actions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::hosts::handlers::finish;
use crate::gui::{GuiApp, emit, window};
use crate::ids::HostId;
use crate::integration::host_app::ProfileRemoval;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

pub(crate) fn on_uninstall(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id.as_str(), "remove", reply_to)
    else {
        return;
    };
    let result = match host.remove_profile() {
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
    };
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
    let Some(host) = crate::gui::hosts::resolve::resolve_or_reply(
        app,
        host_id.as_str(),
        "show config file",
        reply_to,
    ) else {
        return;
    };
    let snapshot = host.probe(&app.probe_env());
    let result = snapshot.profile_source.as_ref().map_or_else(
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
    );
    finish(app, result, reply_to);
}

pub(crate) fn on_open(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id.as_str(), "open", reply_to)
    else {
        return;
    };
    let result = match host.open() {
        Ok(()) => {
            app.append_log(format!("opened host {}", host.display_name()));
            Ok(json!({}))
        },
        Err(err) => {
            let msg = format!("open host {} failed: {err}", host.display_name());
            app.append_log_error(&msg);
            Err(BridgeError::new(ErrorScope::Host, ErrorCode::Internal, msg))
        },
    };
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
