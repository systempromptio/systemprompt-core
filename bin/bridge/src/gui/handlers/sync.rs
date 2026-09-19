//! GUI handlers driving manifest sync and reporting progress.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::gateway::Freshness;
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::{config, i18n, sync};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_sync_requested(app: &mut GuiApp, reply_to: ReplyId) {
    if app.state.snapshot().sync_in_flight {
        // Why: a request with no reply channel comes from login or a gateway
        // change, and dropping it left the new gateway unsynced behind a run
        // for the old one. It waits for that run to finish instead.
        match reply_to {
            Some(id) => {
                let err = BridgeError::new(
                    ErrorScope::Marketplace,
                    ErrorCode::Conflict,
                    "sync already in flight",
                );
                emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
            },
            None => app.state.set_sync_pending(true),
        }
        return;
    }
    app.did_initial_sync = true;
    app.state.set_sync_in_flight(true);
    app.append_log("Sync started…");
    app.refresh_ui();
    emit::emit_sync_progress(app, "started", None);
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Sync);
    let bridge = Arc::clone(&app.ctx);
    {
        let proxy = proxy.clone();
        bridge.sync_progress.install(Arc::new(move |step| {
            proxy.send_event(UiEvent::SyncStep(step.clone()));
        }));
    }
    app.ctx.spawn(async move {
        let allow_tofu = matches!(
            config::pinned_pubkey_state(),
            Ok(config::PinnedPubkeyState::Unpinned)
        );
        // Why: a sync the user pressed usually follows something they just
        // did on the gateway (linking a connector); the memo cannot see it.
        let freshness = if reply_to.is_some() {
            Freshness::Fresh
        } else {
            Freshness::Memo
        };
        let options = sync::SyncOptions {
            allow_unsigned: false,
            force_replay: false,
            allow_tofu,
            freshness,
            cancel: token,
        };
        let result = sync::run_once(&bridge, &options)
            .await
            .map_err(|e| match e {
                sync::SyncError::Cancelled { .. } => GuiError::Cancelled,
                other => GuiError::from(other),
            })
            .map_err(Arc::new);
        bridge.sync_progress.clear();
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
            for warning in &summary.host_warnings {
                app.append_log_warn(format!("[{}] {}", warning.host_id, warning.message));
            }
            emit::emit_sync_progress(app, "completed", Some(&line));
            structured = Some(summary);
            Ok(json!({ "summary": line }))
        },
        Err(msg) if msg.is_cancelled() => {
            let line = i18n::t("sync-cancelled");
            app.append_log(&line);
            emit::emit_sync_progress(app, "cancelled", Some(&line));
            Ok(json!({ "cancelled": true }))
        },
        Err(msg)
            if matches!(
                msg.as_ref(),
                GuiError::Sync(sync::SyncError::Superseded { .. })
            ) =>
        {
            // Why: the run belonged to a gateway the user has left; its
            // outcome is not this gateway's sync failing.
            let line = msg.to_string();
            tracing::info!(%line, "sync superseded");
            app.append_log(&line);
            emit::emit_sync_progress(app, "cancelled", Some(&line));
            Ok(json!({ "superseded": true }))
        },
        Err(msg) => {
            if let GuiError::Sync(sync::SyncError::Partial(summary)) = msg.as_ref() {
                structured = Some(summary.as_ref().clone());
            }
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
            let mut detail = None;
            let (phase, line, scope, code) =
                if let Some(sync::SyncError::Partial(summary)) = sync_err {
                    let (line, code, failures) = partial_failure(app, summary);
                    detail = Some(json!({
                        "host_failures": failures,
                        "host_warnings": summary.host_warnings,
                    }));
                    ("failed", line, ErrorScope::Marketplace, code)
                } else if matches!(sync_err, Some(sync::SyncError::NoCredential { .. })) {
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
            let err = BridgeError::new(scope, code, line);
            Err(match detail {
                Some(detail) => err.with_detail(detail),
                None => err,
            })
        },
    };
    app.state.reload();
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
    app.proxy
        .send_event(UiEvent::ValidateRequested { reply_to: None });
    if succeeded {
        app.proxy.send_event(UiEvent::McpAuthProbeRequested {
            server_id: None,
            reply_to: None,
        });
    }
    if auth_failure && reply_to.is_some() && !app.state.first_run_active() {
        app.proxy.send_event(UiEvent::OpenSettings);
    }
    finish_value(app, bridge_result, reply_to);
    if app.state.take_sync_pending() {
        app.append_log("Sync requested during the previous run; starting it now…");
        app.proxy
            .send_event(UiEvent::SyncRequested { reply_to: None });
    }
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

// Why: every other host applied, so the toast names the agents that did not
// and offers the repair the failure calls for instead of the whole error
// chain; an elevation-required failure is one gesture (UAC) away from fixed.
fn partial_failure(
    app: &GuiApp,
    summary: &sync::SyncSummary,
) -> (String, ErrorCode, Vec<sync::HostFailure>) {
    let failures: Vec<String> = summary
        .host_failures
        .iter()
        .map(|f| format!("{}: {}", f.host_id, f.error.lines().next().unwrap_or("")))
        .collect();
    crate::gui::window::notify_user(
        &format!("{} synced with failures", crate::brand::brand().app_name),
        &format!("These agents did not update — {}", failures.join("; ")),
    );
    for failure in &failures {
        app.append_log_warn(format!("did not update — {failure}"));
    }
    for warning in &summary.host_warnings {
        app.append_log_warn(format!("[{}] {}", warning.host_id, warning.message));
    }
    let needs_elevation = summary.host_failures.iter().any(|f| f.needs_elevation);
    let hosts = summary
        .host_failures
        .iter()
        .map(|f| f.host_id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let count = summary.host_failures.len().to_string();
    let key = if needs_elevation {
        "sync-elevation-required"
    } else {
        "sync-partial"
    };
    let code = if needs_elevation {
        ErrorCode::ElevationRequired
    } else {
        ErrorCode::Partial
    };
    (
        i18n::t_args(key, &[("count", &count), ("hosts", &hosts)]),
        code,
        summary.host_failures.clone(),
    )
}
