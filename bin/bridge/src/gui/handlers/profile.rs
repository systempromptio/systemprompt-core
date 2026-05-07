//! Profile dashboard handler.
//!
//! Composes the response for the bridge dashboard's profile tab from three
//! sources: the cached JWT-derived identity (in `AppStateSnapshot`), the
//! gateway's `/v1/bridge/profile` (gateway-issued user info, allowed models,
//! organization), and `/v1/bridge/profile/usage` (per-user token usage,
//! favorite models, conversation summary). Local agent state is taken from
//! the existing host snapshot so the page works offline for that section.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::AppStateSnapshot;
use crate::gui::{GuiApp, emit};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_profile_fetch_requested(app: &mut GuiApp, reply_to: ReplyId) {
    let snapshot = app.state.snapshot();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = build_profile(snapshot).await.map_err(Arc::new);
        let _ = proxy.send_event(UiEvent::ProfileFetchFinished { result, reply_to });
    });
}

pub(crate) fn on_profile_fetch_finished(
    app: &mut GuiApp,
    result: Result<Value, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(value) => Ok(value),
        Err(err) => {
            let raw = format!("{:#}", err);
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
        .map(|s| s.expose().to_string())
        .ok_or_else(|| {
            GuiError::Io(std::io::Error::other(
                "no valid auth credential available; log in first",
            ))
        })?;

    let whoami = client
        .fetch_whoami(&bearer)
        .await
        .map_err(|e| GuiError::Io(std::io::Error::other(e.to_string())))?;

    let bridge_profile = client.fetch_bridge_profile().await.ok();

    let usage = client.fetch_profile_usage(&bearer).await.ok();

    let agents = agents_summary(&snapshot);
    let identity = identity_value(&snapshot, &whoami);

    Ok(json!({
        "gateway": gateway_url,
        "identity": identity,
        "bridge_profile": bridge_profile,
        "usage": usage,
        "agents": agents,
    }))
}

fn identity_value(
    snapshot: &AppStateSnapshot,
    whoami: &crate::gateway::types::WhoamiResponse,
) -> Value {
    let id = snapshot.verified_identity.as_ref();
    json!({
        "email": whoami.email.clone().or_else(|| id.and_then(|i| i.email.clone())),
        "user_id": whoami.user_id.as_ref().map(|u| u.as_str().to_string())
            .or_else(|| id.and_then(|i| i.user_id.clone())),
        "tenant_id": whoami.tenant_id.as_ref().map(|t| t.as_str().to_string())
            .or_else(|| id.and_then(|i| i.tenant_id.clone())),
        "display_name": whoami.display_name,
        "provider": whoami.provider,
        "roles": whoami.roles,
        "exp_unix": id.and_then(|i| i.exp_unix),
        "verified_at_unix": id.map(|i| i.verified_at_unix),
        "token_length": snapshot.cached_token.as_ref().map(|t| t.length),
        "token_ttl_seconds": snapshot.cached_token.as_ref().map(|t| t.ttl_seconds),
    })
}

fn agents_summary(snapshot: &AppStateSnapshot) -> Value {
    let mut entries = Vec::new();
    for host in crate::integration::host_apps() {
        let id = host.id();
        let st = snapshot.hosts.get(id);
        let probe = st.and_then(|s| s.snapshot.as_ref());
        entries.push(json!({
            "id": id,
            "display_name": host.display_name(),
            "kind": host.kind(),
            "host_running": probe.map(|p| p.host_running).unwrap_or(false),
            "profile_state": probe.map(|p| &p.profile_state),
        }));
    }
    json!({
        "total": entries.len(),
        "items": entries,
    })
}
