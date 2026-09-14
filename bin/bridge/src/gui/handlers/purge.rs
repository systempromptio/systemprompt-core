//! GUI handler for the "remove everything" purge: uninstall, host cleanup and
//! local-state wipe in one step, then back to the first-launch wizard.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::handlers::auth::finish_unit;
use crate::gui::{GuiApp, emit};
use crate::i18n;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

pub(crate) fn on_purge_requested(app: &GuiApp, reply_to: ReplyId) {
    app.state.cancel_all();
    app.append_log(i18n::t("purge-running"));
    let proxy = app.proxy.clone();
    let ctx = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let worker = Arc::clone(&ctx);
        let result = match tokio::task::spawn_blocking(move || {
            crate::integration::uninstall::purge_device(&worker)
                .map(|report| report.leftovers())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        })
        .await
        {
            Ok(r) => r,
            Err(join_err) => Err(Arc::new(GuiError::Io(std::io::Error::other(format!(
                "purge task join: {join_err}"
            ))))),
        };
        proxy.send_event(UiEvent::PurgeFinished { result, reply_to });
    });
}

pub(crate) fn on_disconnect_requested(app: &GuiApp, reply_to: ReplyId) {
    app.state.cancel_all();
    app.append_log(i18n::t("disconnect-running"));
    let proxy = app.proxy.clone();
    let ctx = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let worker = Arc::clone(&ctx);
        let result = match tokio::task::spawn_blocking(move || {
            crate::install::uninstall(false, &worker)
                .map(|_| crate::integration::uninstall::clear_hosts())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        })
        .await
        {
            Ok(r) => r,
            Err(join_err) => Err(Arc::new(GuiError::Io(std::io::Error::other(format!(
                "disconnect task join: {join_err}"
            ))))),
        };
        proxy.send_event(UiEvent::DisconnectFinished { result, reply_to });
    });
}

pub(crate) fn on_purge_finished(
    app: &mut GuiApp,
    result: Result<Vec<String>, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    finish_removal(app, result, reply_to, "purge-success", "purge-failure");
}

pub(crate) fn on_disconnect_finished(
    app: &mut GuiApp,
    result: Result<Vec<String>, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    finish_removal(
        app,
        result,
        reply_to,
        "disconnect-success",
        "disconnect-failure",
    );
}

fn finish_removal(
    app: &mut GuiApp,
    result: Result<Vec<String>, Arc<GuiError>>,
    reply_to: ReplyId,
    success_key: &str,
    failure_key: &str,
) {
    let bridge_result = match result {
        Ok(leftovers) if leftovers.is_empty() => {
            app.append_log(i18n::t(success_key));
            Ok(())
        },
        Ok(leftovers) => {
            let detail = leftovers.join("; ");
            let line = i18n::t_args("removal-incomplete", &[("error", &detail)]);
            app.append_log_error(&line);
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                line,
            ))
        },
        Err(e) => {
            let line = i18n::t_args(failure_key, &[("error", &e.to_string())]);
            app.append_log_error(&line);
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    if !app.reload_runtime_or_fail(reply_to) {
        return;
    }
    app.state.reload();
    app.state.set_agents_onboarded(false);
    if bridge_result.is_ok() {
        app.state.set_pending_device_action(None);
    }
    app.refresh_ui();
    emit::emit_state(app);
    finish_unit(app, bridge_result, reply_to);
}
