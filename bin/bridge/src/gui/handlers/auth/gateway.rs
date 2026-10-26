//! GUI handlers for changing the gateway URL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::finish_unit;
use crate::auth::setup;
use crate::gui::GuiApp;
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::i18n;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_set_gateway_requested(app: &GuiApp, gateway: &str, reply_to: ReplyId) {
    let trimmed = gateway.trim().to_owned();
    if trimmed.is_empty() {
        let msg = i18n::t("gateway-set-empty");
        let err = BridgeError::new(ErrorScope::Gateway, ErrorCode::InvalidArgs, msg.clone());
        app.append_log_error(msg);
        finish_unit(app, Err(err), reply_to);
        return;
    }
    app.append_log(i18n::t_args("gateway-saving", &[("url", &trimmed)]));
    app.state.cancel_scope(CancelScope::Sync);
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::SetGateway);
    app.ctx.spawn(async move {
        let task = tokio::task::spawn_blocking(move || {
            if token.is_cancelled() {
                return Err(Arc::new(GuiError::Cancelled));
            }
            setup::set_gateway_url(&trimmed)
                .map(|_| ())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        });
        let result = match task.await {
            Ok(r) => r,
            Err(join_err) => Err(Arc::new(GuiError::from(setup::SetupError::Join(join_err)))),
        };
        proxy.send_event(UiEvent::SetGatewayFinished { result, reply_to });
    });
}

pub(crate) fn on_set_gateway_finished(
    app: &mut GuiApp,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.clear_cancel(CancelScope::SetGateway);
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(i18n::t("gateway-saved"));
            if !app.reload_runtime_or_fail(reply_to) {
                return;
            }
            // Why: the registry routes /mcp/<name> to the gateway that
            // delivered it; the new gateway's servers arrive with its sync.
            crate::mcp_registry::clear(&app.ctx.mcp_registry);
            app.state.reload();
            crate::gui::handlers::gateway_probe::spawn_probe(app, None);
            Ok(())
        },
        Err(e) if e.is_cancelled() => {
            app.append_log(i18n::t("gateway-set-cancelled"));
            app.state.reload();
            Ok(())
        },
        Err(e) => {
            let line = i18n::t_args("gateway-set-failure", &[("error", &e.to_string())]);
            app.append_log_error(&line);
            app.state.reload();
            Err(BridgeError::new(
                ErrorScope::Gateway,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    app.refresh_ui();
    finish_unit(app, bridge_result, reply_to);
}
