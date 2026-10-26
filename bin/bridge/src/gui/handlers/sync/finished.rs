//! Reporting a finished manifest sync to the GUI and the waiting IPC caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::{i18n, sync};

struct SyncOutcome {
    // JSON: the IPC reply payload the GUI forwards verbatim to the waiting caller.
    reply: Result<serde_json::Value, BridgeError>,
    report: Option<sync::SyncSummary>,
    succeeded: bool,
    cancelled: bool,
    auth_failure: bool,
}

struct FailureMessage {
    line: String,
    code: ErrorCode,
    // JSON: error detail attached to the IPC reply, shaped by the failing stage.
    detail: Option<serde_json::Value>,
}

pub(crate) fn on_sync_finished(
    app: &mut GuiApp,
    result: Result<sync::SyncSummary, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.set_sync_in_flight(false);
    app.state.clear_cancel(CancelScope::Sync);
    let outcome = match result {
        Ok(summary) => report_success(app, summary),
        Err(error) => report_error(app, &error),
    };
    app.state.reload();
    if let Some(summary) = outcome.report {
        app.state.set_last_sync_report(summary);
    }
    app.refresh_ui();
    follow_up(
        app,
        outcome.succeeded,
        outcome.cancelled,
        outcome.auth_failure,
        reply_to,
    );
    finish_value(app, outcome.reply, reply_to);
    if app.state.take_sync_pending() {
        app.append_log("Sync requested during the previous run; starting it now…");
        app.proxy
            .send_event(UiEvent::SyncRequested { reply_to: None });
    }
}

fn report_success(app: &GuiApp, summary: sync::SyncSummary) -> SyncOutcome {
    let line = summary.one_line();
    tracing::info!(summary = %line, "sync completed");
    app.append_log(&line);
    for warning in &summary.host_warnings {
        app.append_log_warn(format!("[{}] {}", warning.host_id, warning.message));
    }
    emit::emit_sync_progress(app, "completed", Some(&line));
    SyncOutcome {
        reply: Ok(json!({ "summary": line })),
        report: Some(summary),
        succeeded: true,
        cancelled: false,
        auth_failure: false,
    }
}

fn report_error(app: &GuiApp, error: &GuiError) -> SyncOutcome {
    if error.is_cancelled() {
        let line = i18n::t("sync-cancelled");
        app.append_log(&line);
        emit::emit_sync_progress(app, "cancelled", Some(&line));
        return stopped(json!({ "cancelled": true }), true);
    }
    if let GuiError::Sync(sync::SyncError::Superseded { .. }) = error {
        // Why: the run belonged to a gateway the user has left; its
        // outcome is not this gateway's sync failing.
        let line = error.to_string();
        tracing::info!(%line, "sync superseded");
        app.append_log(&line);
        emit::emit_sync_progress(app, "cancelled", Some(&line));
        return stopped(json!({ "superseded": true }), false);
    }
    report_failure(app, error)
}

const fn stopped(reply: serde_json::Value, cancelled: bool) -> SyncOutcome {
    SyncOutcome {
        reply: Ok(reply),
        report: None,
        succeeded: false,
        cancelled,
        auth_failure: false,
    }
}

fn report_failure(app: &GuiApp, error: &GuiError) -> SyncOutcome {
    let raw = format!("{error:#}");
    tracing::error!(error = %raw, "sync failed");
    let sync_err = match error {
        GuiError::Sync(e) => Some(e),
        _ => None,
    };
    let report = match sync_err {
        Some(sync::SyncError::Partial(summary)) => Some(summary.as_ref().clone()),
        _ => None,
    };
    let auth_failure = matches!(
        sync_err,
        Some(sync::SyncError::NoCredential { .. } | sync::SyncError::GatewayUnauthorized(_))
    );
    let message = failure_message(app, sync_err, &raw);
    app.append_log_error(&message.line);
    emit::emit_sync_progress(app, "failed", Some(&message.line));
    let err = BridgeError::new(ErrorScope::Marketplace, message.code, message.line);
    SyncOutcome {
        reply: Err(match message.detail {
            Some(detail) => err.with_detail(detail),
            None => err,
        }),
        report,
        succeeded: false,
        cancelled: false,
        auth_failure,
    }
}

fn failure_message(app: &GuiApp, sync_err: Option<&sync::SyncError>, raw: &str) -> FailureMessage {
    let plain = |line: String, code: ErrorCode| FailureMessage {
        line,
        code,
        detail: None,
    };
    match sync_err {
        Some(sync::SyncError::Partial(summary)) => {
            let (line, code, failures) = partial_failure(app, summary);
            FailureMessage {
                line,
                code,
                detail: Some(json!({
                    "host_failures": failures,
                    "host_warnings": summary.host_warnings,
                })),
            }
        },
        Some(sync::SyncError::NoCredential { .. }) => {
            plain(i18n::t("sync-no-credentials"), ErrorCode::Unauthorized)
        },
        Some(sync::SyncError::BridgeTooOld { local, required }) => plain(
            i18n::t_args(
                "sync-bridge-too-old",
                &[("local", local), ("required", required)],
            ),
            ErrorCode::Conflict,
        ),
        Some(sync::SyncError::GatewayUnauthorized(rejection)) => {
            let status_s = rejection.status.to_string();
            plain(
                i18n::t_args(
                    "sync-gateway-unauthorized",
                    &[
                        ("endpoint", rejection.endpoint),
                        ("status", &status_s),
                        ("gateway", &rejection.gateway),
                    ],
                ),
                ErrorCode::Unauthorized,
            )
        },
        _ => plain(
            i18n::t_args("sync-failure", &[("error", raw)]),
            ErrorCode::Internal,
        ),
    }
}

fn follow_up(
    app: &mut GuiApp,
    succeeded: bool,
    cancelled: bool,
    auth_failure: bool,
    reply_to: ReplyId,
) {
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
}
// JSON: webview IPC reply — each command's typed result serialized at the call
// site.
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
