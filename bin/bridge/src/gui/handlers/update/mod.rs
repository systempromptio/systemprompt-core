//! GUI self-update handlers: the tray's check, install and restart. The
//! scheduled trigger lives in [`super::auto_update`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;

use crate::gui::error::GuiError;
use crate::gui::events::{InstalledUpdate, ReplyId, UiEvent};
use crate::gui::{GuiApp, UiEventProxy, emit};
use crate::update::{self, UpdateUiState};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

mod approval;
mod fetch;

pub(crate) use approval::approve_after_update;
use approval::remember_pending_approval;
pub(super) use fetch::client_and_bearer;
use fetch::{check, install};

pub(super) fn progress_reporter(
    proxy: UiEventProxy,
    version: String,
) -> impl Fn(update::DownloadProgress) + Send + Sync {
    move |p: update::DownloadProgress| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "fraction() is clamped to 0.0..=1.0, so the product fits u8"
        )]
        let percent = (p.fraction() * 100.0) as u8;
        proxy.send_event(UiEvent::UpdateProgress {
            version: version.clone(),
            percent,
        });
    }
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_update_check_requested(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = check(http).await.map_err(Arc::new);
        proxy.send_event(UiEvent::UpdateCheckFinished {
            result,
            reply_to,
            automatic: false,
        });
    });
}

pub(crate) fn on_update_check_finished(
    app: &mut GuiApp,
    result: Result<UpdateUiState, Arc<GuiError>>,
    reply_to: ReplyId,
    automatic: bool,
) {
    // Why: a tray click has no reply channel — the toast and the activity
    // log are the only places the person who clicked can see the answer.
    let manual = reply_to.is_none() && !automatic;
    match &result {
        Ok(UpdateUiState::Current) => {
            app.state.apply_update_check(UpdateUiState::Current);
            if manual && !app.state.snapshot().update.can_restart() {
                let version = crate::brand::brand().version;
                app.append_log(format!("up to date (v{version})"));
                crate::gui::window::notify_user(
                    &format!("{} is up to date", crate::brand::brand().app_name),
                    &format!("You are on the latest version, v{version}."),
                );
            }
        },
        Ok(state) => {
            if automatic
                && state.can_install()
                && let Some(version) = state.version()
            {
                app.append_log(format!(
                    "{version} is available; staging it for the next restart"
                ));
            }
            app.state.apply_update_check(state.clone());
        },
        Err(e) => {
            let message = user_message(e);
            tracing::warn!(error = %e, automatic, "update check failed");
            app.append_log(format!("update check failed: {message}"));
            // Why: the next scheduled check retries an automatic one unprompted.
            if !automatic {
                app.state
                    .apply_update_check(UpdateUiState::Failed { message });
            }
            if manual {
                crate::gui::window::notify_user(
                    &format!(
                        "{} couldn't check for updates",
                        crate::brand::brand().app_name
                    ),
                    "Check your connection and try again from the tray menu.",
                );
            }
        },
    }
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update check");
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_update_install_requested(
    app: &mut GuiApp,
    event_loop: &dyn ActiveEventLoop,
    reply_to: ReplyId,
) {
    if app.state.snapshot().update.can_restart() {
        if let Some(id) = reply_to {
            emit::send_reply_payload(app, id, &IpcReplyPayload::ok(serde_json::Value::Null));
        }
        on_update_restart_requested(app, event_loop);
        return;
    }
    if app.state.snapshot().update.in_progress() {
        if let Some(id) = reply_to {
            emit::send_reply_payload(app, id, &IpcReplyPayload::ok(serde_json::Value::Null));
        }
        return;
    }
    let Some(version) = app.state.snapshot().update.version().map(str::to_owned) else {
        if let Some(id) = reply_to {
            let payload = IpcReplyPayload::err(BridgeError::new(
                ErrorScope::Internal,
                ErrorCode::Internal,
                "no update is pending".to_owned(),
            ));
            emit::send_reply_payload(app, id, &payload);
        }
        return;
    };

    app.state.set_update_state(UpdateUiState::Downloading {
        version: version.clone(),
        percent: 0,
    });
    app.refresh_ui();
    emit::emit_state(app);

    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    let finished = proxy.clone();
    let completion =
        update::completion::Completion::new(move |result: Result<InstalledUpdate, GuiError>| {
            finished.send_event(UiEvent::UpdateInstallFinished {
                result: result.map_err(Arc::new),
                reply_to,
            });
        });
    app.ctx.spawn(async move {
        let on_progress = progress_reporter(proxy, version.clone());
        completion.finish(install(&version, http, &on_progress).await);
    });
}

pub(crate) fn on_update_progress(app: &mut GuiApp, version: &str, percent: u8) {
    app.state.set_update_progress(version, percent);
    app.refresh_ui();
    emit::emit_state(app);
}

pub(crate) fn on_update_install_finished(
    app: &mut GuiApp,
    result: Result<InstalledUpdate, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    match &result {
        Ok(installed) => {
            let version = installed.version.clone();
            let app_name = crate::brand::brand().app_name;
            crate::gui::window::notify_user(
                &format!("{app_name} {version} is installed"),
                &format!(
                    "Restart {app_name} from the tray or account menu to finish installing \
                     {version}. Otherwise it finishes the next time {app_name} starts."
                ),
            );
            app.state.set_update_state(UpdateUiState::Ready { version });
        },
        Err(e) => {
            let message = user_message(e);
            tracing::error!(error = %e, automatic = reply_to.is_none(), "update install failed");
            app.append_log(format!("update failed: {message}"));
            let state = match e.as_ref() {
                GuiError::Update(update::UpdateError::InstalledUnrecorded { version, .. }) => {
                    UpdateUiState::Ready {
                        version: version.clone(),
                    }
                },
                _ => UpdateUiState::Failed { message },
            };
            app.state.set_update_state(state);
        },
    }
    app.refresh_ui();
    emit::emit_state(app);
    reply(app, reply_to, result, "update install");
}

pub(crate) fn on_update_restart_requested(app: &mut GuiApp, event_loop: &dyn ActiveEventLoop) {
    let installed = match update::installed_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "update: cannot resolve the installed path to relaunch");
            app.append_log(format!("restart failed: {}", e.user_message()));
            return;
        },
    };
    stop_serving(app);
    if let Err(e) = update::spawn_successor(&installed) {
        tracing::error!(error = %e, "update: relaunch failed; leaving this instance running");
        app.append_log(format!(
            "restart failed: {}; the update is installed — reopen manually",
            e.user_message()
        ));
        return;
    }
    crate::gui::handlers::quit::on_quit(app, event_loop);
}

pub(crate) fn on_handoff_requested(app: &mut GuiApp, event_loop: &dyn ActiveEventLoop) {
    tracing::info!("handoff: stopping so the newer build can start");
    app.append_log("a newer build was opened; handing over to it");
    stop_serving(app);
    crate::gui::handlers::quit::on_quit(app, event_loop);
}

// Why: the successor binds the default proxy port, so this instance stops
// serving before it exits; host profiles written for 48217 are rejected
// against any other port.
fn stop_serving(app: &GuiApp) {
    let drained = app
        .ctx
        .proxy
        .served()
        .is_none_or(|served| served.drain(crate::proxy::DRAIN_DEADLINE));
    if !drained {
        app.append_log("restart: proxy requests were still in flight at the drain deadline");
    }
    remember_pending_approval(&app.ctx.elevation_gate);
}

fn reply<T: serde::Serialize>(
    app: &GuiApp,
    reply_to: ReplyId,
    result: Result<T, Arc<GuiError>>,
    what: &str,
) {
    if reply_to.is_none() {
        return;
    }
    let result = result.map_err(|err| {
        tracing::warn!(error = %err, operation = what, "update operation failed");
        BridgeError::new(
            ErrorScope::Internal,
            ErrorCode::Internal,
            user_message(&err),
        )
    });
    emit::finish(app, reply_to, result);
}

fn user_message(err: &GuiError) -> String {
    match err {
        GuiError::Update(update) => update.user_message(),
        other => other.to_string(),
    }
}
