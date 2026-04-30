use std::sync::Arc;

use serde_json::json;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, ipc_runtime};
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
            ipc_runtime::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    app.state.set_sync_in_flight(true);
    app.append_log("Sync started…");
    app.refresh_ui();
    ipc_runtime::emit_sync_progress(app, "started", None);
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Sync);
    app.runtime.spawn(async move {
        let task = tokio::task::spawn_blocking(|| {
            let allow_tofu = config::pinned_pubkey().is_none();
            sync::run_once(false, false, allow_tofu)
                .map_err(GuiError::from)
                .map_err(Arc::new)
        });
        let result = tokio::select! {
            _ = token.cancelled() => {
                Err(Arc::new(GuiError::Io(std::io::Error::other("sync cancelled"))))
            }
            joined = task => match joined {
                Ok(r) => r,
                Err(join_err) => Err(Arc::new(GuiError::Io(std::io::Error::other(format!(
                    "sync task join: {join_err}"
                ))))),
            },
        };
        let _ = proxy.send_event(UiEvent::SyncFinished { result, reply_to });
    });
}

pub(crate) fn on_sync_started(app: &mut GuiApp) {
    app.state.set_sync_in_flight(true);
    app.refresh_ui();
}

pub(crate) fn on_sync_finished(
    app: &mut GuiApp,
    result: Result<crate::sync::SyncSummary, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.set_sync_in_flight(false);
    app.state.clear_cancel(CancelScope::Sync);
    let bridge_result = match result {
        Ok(summary) => {
            let line = summary.one_line();
            app.append_log(&line);
            ipc_runtime::emit_sync_progress(app, "completed", Some(&line));
            Ok(json!({ "summary": line }))
        },
        Err(msg) => {
            let raw = msg.to_string();
            let cancelled = raw.contains("sync cancelled");
            let (phase, line, scope, code) = if cancelled {
                (
                    "cancelled",
                    i18n::t("sync-cancelled"),
                    ErrorScope::Marketplace,
                    ErrorCode::Conflict,
                )
            } else {
                (
                    "failed",
                    i18n::t_args("sync-failure", &[("error", &raw)]),
                    ErrorScope::Marketplace,
                    ErrorCode::Internal,
                )
            };
            app.append_log(&line);
            ipc_runtime::emit_sync_progress(app, phase, Some(&line));
            Err(BridgeError::new(scope, code, line))
        },
    };
    app.state.reload();
    app.refresh_ui();
    ipc_runtime::emit_state(app);
    finish_value(app, bridge_result, reply_to);
}

fn finish_value(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            ipc_runtime::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            ipc_runtime::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    ipc_runtime::send_reply_payload(app, id, &payload);
}
