//! Session-based device login for the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::auth::setup;
use crate::gui::GuiApp;
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::i18n;

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
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = run_session_login(gateway, keep_signed_in, &cancel, http)
            .await
            .map_err(GuiError::from)
            .map_err(Arc::new);
        proxy.send_event(UiEvent::SessionLoginFinished { result, reply_to });
    });
}

async fn run_session_login(
    gateway: Option<String>,
    keep_signed_in: bool,
    cancel: &tokio_util::sync::CancellationToken,
    http: reqwest::Client,
) -> Result<(), setup::SetupError> {
    use crate::auth::providers::session::capture_device_link_code;
    use crate::gateway::GatewayClient;
    use crate::gateway::types::{HelperOutput, SessionExchangeRequest, SessionPatRequest};
    use systemprompt_identifiers::SessionId;

    if let Some(g) = gateway.clone()
        && !g.trim().is_empty()
    {
        tokio::task::spawn_blocking(move || setup::set_gateway_url(&g)).await??;
    }
    let cfg = crate::config::load();
    let base = crate::config::gateway_url_or_default(&cfg);
    let session_id = SessionId::generate();

    let code = tokio::select! {
        () = cancel.cancelled() => {
            return Err(setup::SetupError::Cancelled);
        }
        result = capture_device_link_code(&base) => {
            result.map_err(|e| setup::SetupError::Io(e.to_string()))?
        }
    };

    let client = GatewayClient::new(base.clone(), http);
    if keep_signed_in {
        let req = SessionPatRequest {
            code,
            device_name: Some(default_device_name()),
        };
        let pat = client.session_pat_exchange(&req, &session_id).await?;
        let gw = gateway.clone();
        tokio::task::spawn_blocking(move || setup::login(pat.as_str(), gw.as_deref())).await??;
    } else {
        let req = SessionExchangeRequest { code };
        let out: HelperOutput = client.session_exchange(&req, &session_id).await?.into();
        let gw = gateway.clone();
        tokio::task::spawn_blocking(move || setup::session_setup(gw.as_deref())).await??;
        if let Err(e) = crate::auth::cache::write(&base, &out) {
            crate::stdio::diag(&format!("session cache write failed (continuing): {e}"));
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
