//! Profile dashboard handler: composes the dashboard profile tab from the
//! cached JWT identity, the gateway profile, and per-user usage.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::AppStateSnapshot;
use crate::gui::{GuiApp, emit};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_profile_fetch_requested(app: &GuiApp, reply_to: ReplyId) {
    let snapshot = app.state.snapshot();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = build_profile(snapshot).await.map_err(Arc::new);
        _ = proxy.send_event(UiEvent::ProfileFetchFinished { result, reply_to });
    });
}

pub(crate) fn on_profile_fetch_finished(
    app: &GuiApp,
    result: Result<Value, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(value) => Ok(value),
        Err(err) => {
            let raw = format!("{err:#}");
            tracing::error!(error = %raw, "profile fetch failed");
            app.append_log(format!("profile fetch failed: {raw}"));
            Err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                raw,
            ))
        },
    };
    let Some(id) = reply_to else {
        if let Err(err) = bridge_result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match bridge_result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            emit::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}

async fn build_profile(snapshot: AppStateSnapshot) -> Result<Value, GuiError> {
    use crate::config;
    use crate::gateway::GatewayClient;

    let cfg = config::load();
    let gateway_url = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway_url.clone());

    let bearer_value = crate::auth::cache::read_valid().map(|out| out.token);
    let bearer = bearer_value
        .as_ref()
        .map(|s| s.expose().to_owned())
        .ok_or_else(|| {
            GuiError::Io(std::io::Error::other(
                "no valid auth credential available; log in first",
            ))
        })?;

    let whoami = match client.fetch_whoami(&bearer).await {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(error = %e, "whoami enrichment failed; falling back to snapshot identity");
            None
        },
    };

    let bridge_profile = client.fetch_bridge_profile().await.ok();

    let usage = client.fetch_profile_usage(&bearer).await.ok();

    let identity = identity_value(&snapshot, whoami.as_ref());

    Ok(json!({
        "gateway": gateway_url,
        "identity": identity,
        "bridge_profile": bridge_profile,
        "usage": usage,
    }))
}

fn identity_value(
    snapshot: &AppStateSnapshot,
    whoami: Option<&crate::gateway::types::WhoamiResponse>,
) -> Value {
    let id = snapshot.verified_identity.as_ref();
    json!({
        "email": whoami.and_then(|w| w.email.clone())
            .or_else(|| id.and_then(|i| i.email.clone())),
        "user_id": whoami.and_then(|w| w.user_id.as_ref().map(|u| u.as_str().to_owned()))
            .or_else(|| id.and_then(|i| i.user_id.as_ref().map(|u| u.as_str().to_owned()))),
        "tenant_id": whoami.and_then(|w| w.tenant_id.as_ref().map(|t| t.as_str().to_owned()))
            .or_else(|| id.and_then(|i| i.tenant_id.as_ref().map(|t| t.as_str().to_owned()))),
        "display_name": whoami.and_then(|w| w.display_name.clone()),
        "provider": whoami.and_then(|w| w.provider.clone()),
        "roles": whoami.map(|w| w.roles.clone()).unwrap_or_default(),
        "exp_unix": id.and_then(|i| i.exp_unix),
        "verified_at_unix": id.map(|i| i.verified_at_unix),
        "token_length": snapshot.cached_token.as_ref().map(|t| t.length),
        "token_ttl_seconds": snapshot.cached_token.as_ref().map(|t| t.ttl_seconds),
    })
}
