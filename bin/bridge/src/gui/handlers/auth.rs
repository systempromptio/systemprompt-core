//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::auth::secret::Secret;
use crate::auth::setup;
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::i18n;

#[tracing::instrument(level = "info", skip(app, token), fields(has_gateway = gateway.is_some()))]
pub(crate) fn on_login_requested(
    app: &GuiApp,
    token: &Secret,
    gateway: Option<String>,
    reply_to: ReplyId,
) {
    let trimmed = Secret::new(token.expose().trim().to_owned());
    if trimmed.is_empty() {
        let msg = i18n::t("login-pat-empty");
        let err = BridgeError::new(ErrorScope::Identity, ErrorCode::InvalidArgs, msg.clone());
        app.append_log(msg);
        finish_unit(app, Err(err), reply_to);
        return;
    }
    app.append_log(i18n::t("login-saving"));
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Login);
    app.runtime.spawn(async move {
        let task = tokio::task::spawn_blocking(move || {
            setup::login(trimmed.expose(), gateway.as_deref())
                .map(|_| ())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        });
        let result = tokio::select! {
            () = token.cancelled() => {
                Err(Arc::new(GuiError::from(setup::SetupError::Io("login cancelled".into()))))
            }
            joined = task => match joined {
                Ok(r) => r,
                Err(join_err) => Err(Arc::new(GuiError::from(setup::SetupError::Io(format!(
                    "login task join: {join_err}"
                ))))),
            },
        };
        _ = proxy.send_event(UiEvent::LoginFinished { result, reply_to });
    });
}

#[tracing::instrument(level = "info", skip(app), fields(has_gateway = gateway.is_some(), keep_signed_in))]
pub(crate) fn on_session_login_requested(
    app: &GuiApp,
    gateway: Option<String>,
    keep_signed_in: bool,
    reply_to: ReplyId,
) {
    app.append_log(i18n::t("login-saving"));
    let proxy = app.proxy.clone();
    let cancel = app.state.install_cancel(CancelScope::Login);
    app.runtime.spawn(async move {
        let result = run_session_login(gateway, keep_signed_in, &cancel)
            .await
            .map_err(GuiError::from)
            .map_err(Arc::new);
        _ = proxy.send_event(UiEvent::SessionLoginFinished { result, reply_to });
    });
}

async fn run_session_login(
    gateway: Option<String>,
    keep_signed_in: bool,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), setup::SetupError> {
    use crate::auth::providers::session::capture_device_link_code;
    use crate::auth::types::{HelperOutput, SessionExchangeRequest, SessionPatRequest};
    use crate::gateway::GatewayClient;
    use systemprompt_identifiers::SessionId;

    if let Some(g) = gateway.clone()
        && !g.trim().is_empty()
    {
        tokio::task::spawn_blocking(move || setup::set_gateway_url(&g))
            .await
            .map_err(|e| setup::SetupError::Io(format!("set-gateway join: {e}")))??;
    }
    let cfg = crate::config::load();
    let base = crate::config::gateway_url_or_default(&cfg);
    let session_id = SessionId::generate();

    let code = tokio::select! {
        () = cancel.cancelled() => {
            return Err(setup::SetupError::Io("sign-in cancelled".into()));
        }
        result = capture_device_link_code(&base) => {
            result.map_err(|e| setup::SetupError::Io(e.to_string()))?
        }
    };

    let client = GatewayClient::new(base);
    if keep_signed_in {
        let req = SessionPatRequest {
            code,
            device_name: Some(default_device_name()),
        };
        let pat = client
            .session_pat_exchange(&req, &session_id)
            .await
            .map_err(|e| setup::SetupError::Io(e.to_string()))?;
        let gw = gateway.clone();
        tokio::task::spawn_blocking(move || setup::login(&pat, gw.as_deref()))
            .await
            .map_err(|e| setup::SetupError::Io(format!("login join: {e}")))??;
    } else {
        let req = SessionExchangeRequest { code };
        let out: HelperOutput = client
            .session_exchange(&req, &session_id)
            .await
            .map_err(|e| setup::SetupError::Io(e.to_string()))?
            .into();
        let gw = gateway.clone();
        tokio::task::spawn_blocking(move || setup::session_setup(gw.as_deref()))
            .await
            .map_err(|e| setup::SetupError::Io(format!("session setup join: {e}")))??;
        if let Err(e) = crate::auth::cache::write(&out) {
            crate::obs::output::diag(&format!("session cache write failed (continuing): {e}"));
        }
    }
    Ok(())
}

fn default_device_name() -> String {
    let host = std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "device".to_owned());
    format!("{} — {host}", crate::brand::brand().app_name)
}

pub(crate) fn on_login_finished(
    app: &mut GuiApp,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.clear_cancel(CancelScope::Login);
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(i18n::t("login-pull-manifest"));
            crate::proxy::reload_runtime_config();
            super::gateway_probe::spawn_probe(app, None);
            app.state.reload();
            app.refresh_ui();
            _ = app
                .proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            crate::gui::hosts::tick::request_initial_probe(app);
            Ok(())
        },
        Err(e) => {
            let raw = e.to_string();
            let key = if raw.contains("login cancelled") {
                "login-cancelled"
            } else {
                "login-failure"
            };
            let line = i18n::t_args(key, &[("error", &raw)]);
            app.append_log(&line);
            app.state.reload();
            app.refresh_ui();
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Unauthorized,
                line,
            ))
        },
    };
    finish_unit(app, bridge_result, reply_to);
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_set_gateway_requested(app: &GuiApp, gateway: &str, reply_to: ReplyId) {
    let trimmed = gateway.trim().to_owned();
    if trimmed.is_empty() {
        let msg = i18n::t("gateway-set-empty");
        let err = BridgeError::new(ErrorScope::Gateway, ErrorCode::InvalidArgs, msg.clone());
        app.append_log(msg);
        finish_unit(app, Err(err), reply_to);
        return;
    }
    app.append_log(i18n::t_args("gateway-saving", &[("url", &trimmed)]));
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Login);
    app.runtime.spawn(async move {
        let task = tokio::task::spawn_blocking(move || {
            setup::set_gateway_url(&trimmed)
                .map(|_| ())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        });
        let result = tokio::select! {
            () = token.cancelled() => {
                Err(Arc::new(GuiError::from(setup::SetupError::Io(
                    "set-gateway cancelled".into(),
                ))))
            }
            joined = task => match joined {
                Ok(r) => r,
                Err(join_err) => Err(Arc::new(GuiError::from(setup::SetupError::Io(format!(
                    "set-gateway task join: {join_err}"
                ))))),
            },
        };
        _ = proxy.send_event(UiEvent::SetGatewayFinished { result, reply_to });
    });
}

pub(crate) fn on_set_gateway_finished(
    app: &mut GuiApp,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    app.state.clear_cancel(CancelScope::Login);
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(i18n::t("gateway-saved"));
            crate::proxy::reload_runtime_config();
            app.state.reload();
            super::gateway_probe::spawn_probe(app, None);
            Ok(())
        },
        Err(e) => {
            let line = i18n::t_args("gateway-set-failure", &[("error", &e.to_string())]);
            app.append_log(&line);
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

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_logout_requested(app: &GuiApp, reply_to: ReplyId) {
    app.append_log(i18n::t("logout-running"));
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = match tokio::task::spawn_blocking(|| {
            setup::logout()
                .map(|_| ())
                .map_err(GuiError::from)
                .map_err(Arc::new)
        })
        .await
        {
            Ok(r) => r,
            Err(join_err) => Err(Arc::new(GuiError::from(setup::SetupError::Io(format!(
                "logout task join: {join_err}"
            ))))),
        };
        _ = proxy.send_event(UiEvent::LogoutFinished { result, reply_to });
    });
}

pub(crate) fn on_logout_finished(
    app: &mut GuiApp,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            let msg = i18n::t("logout-success");
            app.append_log(&msg);
            Ok(())
        },
        Err(e) => {
            let line = i18n::t_args("logout-failure", &[("error", &e.to_string())]);
            app.append_log(&line);
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    app.state.reload();
    app.refresh_ui();
    emit::emit_state(app);
    finish_unit(app, bridge_result, reply_to);
}

fn finish_unit(app: &GuiApp, result: Result<(), BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(()) => crate::gui::ipc::IpcReplyPayload::ok(json!({})),
        Err(err) => {
            emit::emit_error(app, &err);
            crate::gui::ipc::IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
