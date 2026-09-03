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
    app.append_log(i18n::t("purge-running"));
    let proxy = app.proxy.clone();
    let ctx = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let worker = Arc::clone(&ctx);
        let result = match tokio::task::spawn_blocking(move || {
            crate::integration::uninstall::purge_device(&worker)
                .map(|_| ())
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

pub(crate) fn on_purge_finished(
    app: &mut GuiApp,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(i18n::t("purge-success"));
            Ok(())
        },
        Err(e) => {
            let line = i18n::t_args("purge-failure", &[("error", &e.to_string())]);
            app.append_log_error(&line);
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    // Why: a failed purge may still have removed the credential or the
    // sentinels, so the runtime and the snapshot are rebuilt from disk either
    // way rather than trusting what the UI showed before the click.
    app.ctx.proxy.reload_runtime_config();
    app.state.reload();
    app.state.set_agents_onboarded(false);
    app.refresh_ui();
    emit::emit_state(app);
    finish_unit(app, bridge_result, reply_to);
}
