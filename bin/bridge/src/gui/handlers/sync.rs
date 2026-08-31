//! GUI handlers driving manifest sync and reporting progress.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::{config, i18n, sync};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_sync_requested(app: &mut GuiApp, reply_to: ReplyId) {
    if app.state.snapshot().sync_in_flight {
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Marketplace,
                ErrorCode::Conflict,
                "sync already in flight",
            );
            emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    app.state.set_sync_in_flight(true);
    app.append_log("Sync started…");
    app.refresh_ui();
    emit::emit_sync_progress(app, "started", None);
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Sync);
    app.runtime.spawn(async move {
        let allow_tofu = config::pinned_pubkey().is_none();
        let result = tokio::select! {
            () = token.cancelled() => Err(Arc::new(GuiError::Cancelled)),
            outcome = sync::run_once(false, false, allow_tofu) => {
                outcome.map_err(GuiError::from).map_err(Arc::new)
            }
        };
        proxy.send_event(UiEvent::SyncFinished { result, reply_to });
    });
}

pub(crate) fn on_sync_started(app: &mut GuiApp) {
    app.state.set_sync_in_flight(true);
    app.refresh_ui();
}

pub(crate) fn on_sync_finished(
    app: &mut GuiApp,
    result: Result<sync::SyncSummary, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.set_sync_in_flight(false);
    app.state.clear_cancel(CancelScope::Sync);
    let succeeded = result.is_ok();
    let cancelled = result.as_ref().err().is_some_and(|e| e.is_cancelled());
    let mut auth_failure = false;
    let mut structured = None;
    let bridge_result = match result {
        Ok(summary) => {
            let line = summary.one_line();
            tracing::info!(summary = %line, "sync completed");
            app.append_log(&line);
            if !summary.host_failures.is_empty() {
                let hosts: Vec<&str> = summary
                    .host_failures
                    .iter()
                    .map(|f| f.host_id.as_str())
                    .collect();
                crate::gui::window::notify_user(
                    &format!("{} synced with failures", crate::brand::brand().app_name),
                    &format!("These agents did not update: {}", hosts.join(", ")),
                );
                app.append_log_warn(format!("These agents did not update: {}", hosts.join(", ")));
            }
            emit::emit_sync_progress(app, "completed", Some(&line));
            structured = Some(summary);
            Ok(json!({ "summary": line }))
        },
        Err(msg) if msg.is_cancelled() => {
            // Why: a cancelled sync is not a failed sync. It reports the
            // `cancelled` phase so the UI settles, and replies Ok so no error
            // toast is raised for something the user (or a newer sync) asked for.
            let line = i18n::t("sync-cancelled");
            app.append_log(&line);
            emit::emit_sync_progress(app, "cancelled", Some(&line));
            Ok(json!({ "cancelled": true }))
        },
        Err(msg) => {
            let raw = format!("{msg:#}");
            tracing::error!(error = %raw, "sync failed");
            let sync_err = match msg.as_ref() {
                GuiError::Sync(e) => Some(e),
                _ => None,
            };
            auth_failure = matches!(
                sync_err,
                Some(
                    sync::SyncError::NoCredential { .. } | sync::SyncError::GatewayUnauthorized(_)
                )
            );
            let (phase, line, scope, code) =
                if matches!(sync_err, Some(sync::SyncError::NoCredential { .. })) {
                    (
                        "failed",
                        i18n::t("sync-no-credentials"),
                        ErrorScope::Marketplace,
                        ErrorCode::Unauthorized,
                    )
                } else if let Some(sync::SyncError::BridgeTooOld { local, required }) = sync_err {
                    (
                        "failed",
                        i18n::t_args(
                            "sync-bridge-too-old",
                            &[("local", local), ("required", required)],
                        ),
                        ErrorScope::Marketplace,
                        ErrorCode::Conflict,
                    )
                } else if let Some(sync::SyncError::GatewayUnauthorized(rejection)) = sync_err {
                    let status_s = rejection.status.to_string();
                    (
                        "failed",
                        i18n::t_args(
                            "sync-gateway-unauthorized",
                            &[
                                ("endpoint", rejection.endpoint),
                                ("status", &status_s),
                                ("gateway", &rejection.gateway),
                            ],
                        ),
                        ErrorScope::Marketplace,
                        ErrorCode::Unauthorized,
                    )
                } else {
                    (
                        "failed",
                        i18n::t_args("sync-failure", &[("error", &raw)]),
                        ErrorScope::Marketplace,
                        ErrorCode::Internal,
                    )
                };
            app.append_log_error(&line);
            emit::emit_sync_progress(app, phase, Some(&line));
            Err(BridgeError::new(scope, code, line))
        },
    };
    app.state.reload();
    // Why: `reload` re-derives `last_sync_summary` from the on-disk sentinel, so
    // the structured report has to be stored after it, not before.
    if let Some(summary) = structured {
        app.state.set_last_sync_report(summary);
    }
    app.refresh_ui();
    if app.state.first_run_active() {
        if cancelled {
            crate::gui::first_run::handlers::on_sync_cancelled(app);
        } else {
            crate::gui::first_run::handlers::on_sync_result(app, succeeded);
        }
    }
    emit::emit_state(app);
    if succeeded {
        app.proxy
            .send_event(UiEvent::McpAuthProbeRequested { reply_to: None });
    }
    // Why: only a user-initiated sync (reply_to set) may raise the settings
    // window for re-auth; a background sync popping a window would steal focus.
    if auth_failure && reply_to.is_some() && !app.state.first_run_active() {
        app.proxy.send_event(UiEvent::OpenSettings);
    }
    finish_value(app, bridge_result, reply_to);
}

fn finish_value(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
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
