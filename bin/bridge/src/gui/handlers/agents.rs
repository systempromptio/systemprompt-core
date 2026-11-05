//! GUI handlers for host-app install/uninstall/open actions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use systemprompt_models::bridge::host::HostKind;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::hosts::handlers::finish;
use crate::gui::{GuiApp, emit, window};
use crate::integration::host_app::ProfileRemoval;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

// Why: the removal waits on the administrator prompt, which must not hold the
// event loop.
fn remove_disabled_desktop_policy(app: &GuiApp, reply_to: ReplyId) {
    let ctx = Arc::clone(&app.ctx);
    let proxy = app.proxy.clone();
    app.ctx.spawn(async move {
        let result =
            tokio::task::spawn_blocking(move || crate::install::remove_managed_profile(&ctx))
                .await
                .map(Arc::new)
                .map_err(|e| Arc::new(GuiError::Io(std::io::Error::other(e))));
        proxy.send_event(UiEvent::Host(HostUiEvent::PolicyRemovalFinished {
            result,
            reply_to,
        }));
    });
}

pub(crate) fn on_policy_removal_finished(
    app: &GuiApp,
    result: &Result<Arc<crate::install::ManagedProfileOutcome>, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    use crate::install::ManagedProfileOutcome;

    let failure = match result.as_ref().map(Arc::as_ref) {
        Ok(ManagedProfileOutcome::RemoveFailed(e)) => Some(e.to_string()),
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    };
    let reply = failure.map_or_else(
        || {
            app.append_log("[claude-desktop] policy removed");
            app.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            Ok(json!({ "removed": true }))
        },
        |cause| {
            app.append_log_error(format!("[claude-desktop] policy removal failed: {cause}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                "could not remove the Claude Desktop policy",
            ))
        },
    );
    finish(app, reply, reply_to);
}

pub(crate) fn on_uninstall(app: &GuiApp, host_id: HostKind, reply_to: ReplyId) {
    let Some(host) = crate::gui::hosts::resolve::resolve_or_reply(app, host_id, "remove", reply_to)
    else {
        return;
    };
    let enabled = app.state.snapshot().enabled_hosts;
    if host_id == HostKind::ClaudeDesktop && !enabled.contains(&host_id) {
        remove_disabled_desktop_policy(app, reply_to);
        return;
    }
    let removal = crate::integration::enrol::retained_for_sibling(host_id, &[host_id], &enabled)
        .map_or_else(
            || host.remove_profile(),
            |sibling| Ok(ProfileRemoval::RetainedForSibling { sibling }),
        );
    let result = match removal {
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
        Ok(ProfileRemoval::RetainedForSibling { sibling }) => {
            app.append_log(format!(
                "[{host_id}] shares its configuration with {sibling}, which is still enabled; \
                 nothing removed"
            ));
            Ok(json!({ "removed": false, "retained_for": sibling }))
        },
        Err(e) => {
            app.append_log_error(format!("[{host_id}] removal failed: {e}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                if e.is_refusal() || e.is_permission_denied() {
                    ErrorCode::Unauthorized
                } else {
                    ErrorCode::Internal
                },
                format!("could not remove {}: {e}", host.display_name()),
            ))
        },
    };
    if result.is_ok() {
        app.proxy
            .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
                host_id,
                cause: crate::gui::hosts::events::ProbeCause::Manual,
                reply_to: None,
            }));
    }
    finish(app, result, reply_to);
}

pub(crate) fn on_open_config(app: &GuiApp, host_id: HostKind, reply_to: ReplyId) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id, "show config file", reply_to)
    else {
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

pub(crate) fn on_open(app: &GuiApp, host_id: HostKind, reply_to: ReplyId) {
    let Some(host) = crate::gui::hosts::resolve::resolve_or_reply(app, host_id, "open", reply_to)
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

pub(crate) fn on_enrol(app: &mut GuiApp, host_id: HostKind, reply_to: ReplyId) {
    if host_id != HostKind::ClaudeCode {
        finish(
            app,
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::InvalidArgs,
                format!("{host_id} is not enrolled through its settings file"),
            )),
            reply_to,
        );
        return;
    }
    let report = crate::integration::enrol::claude_code::enrol_report(&app.ctx);
    let result = match report.outcome {
        crate::integration::enrol::Outcome::Failed(err)
            if crate::integration::claude_code_routing::is_routed() =>
        {
            app.append_log_warn(format!(
                "[{host_id}] routed through the gateway by the user settings; the managed \
                 settings were not written: {err}"
            ));
            Ok(json!({ "enrolled": true }))
        },
        crate::integration::enrol::Outcome::Failed(err) => {
            app.append_log_error(format!("[{host_id}] gateway routing not written: {err}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                format!(
                    "could not route {} through the gateway: {err}",
                    report.display_name
                ),
            ))
        },
        _ => {
            app.append_log(format!("[{host_id}] {}", report.install_action_label));
            Ok(json!({ "enrolled": true }))
        },
    };
    emit::emit_state(app);
    finish(app, result, reply_to);
}

pub(crate) fn on_setup_complete(app: &mut GuiApp) {
    if app.state.first_run_active() {
        return;
    }
    app.state.set_agents_onboarded(true);
    crate::gui::onboarding::mark_complete();
    app.append_log("setup marked complete by user");
    emit::emit_state(app);
}
