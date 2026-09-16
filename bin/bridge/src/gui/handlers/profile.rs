//! Composes the dashboard profile tab from cached identity, gateway profile,
//! and usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::AppStateSnapshot;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};
use crate::wire::profile::{ProfileIdentity, ProfileView};

#[must_use]
pub const fn is_logged_out_error(err: &GuiError) -> bool {
    matches!(err, GuiError::NotAuthenticated)
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_profile_fetch_requested(app: &GuiApp, reply_to: ReplyId) {
    let snapshot = app.state.snapshot();
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = Box::new(build_profile(snapshot, http).await.map_err(Arc::new));
        proxy.send_event(UiEvent::ProfileFetchFinished { result, reply_to });
    });
}

pub(crate) fn on_profile_fetch_finished(
    app: &GuiApp,
    result: Result<ProfileView, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    if matches!(&result, Err(e) if is_logged_out_error(e.as_ref())) {
        tracing::debug!("profile fetch skipped: not authenticated yet");
        if let Some(target) = reply_to {
            let payload = crate::wire::ipc::IpcReplyPayload::err(BridgeError::new(
                ErrorScope::Identity,
                ErrorCode::Internal,
                "not authenticated".to_owned(),
            ));
            emit::send_reply_payload(app, target, &payload);
        }
        return;
    }

    let bridge_result = result.map_err(|err| {
        let raw = format!("{err:#}");
        tracing::error!(error = %raw, "profile fetch failed");
        app.append_log_error(format!("profile fetch failed: {raw}"));
        BridgeError::new(ErrorScope::Identity, ErrorCode::Internal, raw)
    });
    emit::finish(app, reply_to, bridge_result);
}

async fn build_profile(
    snapshot: AppStateSnapshot,
    http: reqwest::Client,
) -> Result<ProfileView, GuiError> {
    use crate::config;
    use crate::gateway::GatewayClient;

    let cfg = config::load()?;
    let gateway_url = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway_url.clone(), http);

    let bearer_value = crate::auth::cache::read_for(&cfg, &gateway_url, 30)
        .map_err(|e| GuiError::Profile {
            context: "credential cache".into(),
            source: e,
        })?
        .map(|out| out.token);
    let bearer = bearer_value.ok_or(GuiError::NotAuthenticated)?;

    let whoami = match client.fetch_whoami(&bearer).await {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(error = %e, "whoami enrichment failed; falling back to snapshot identity");
            None
        },
    };

    let bridge_profile = client.fetch_bridge_profile().await?;
    let usage = client.fetch_profile_usage(&bearer).await?;
    let identity = profile_identity(&snapshot, whoami.as_ref());

    Ok(ProfileView {
        gateway: gateway_url.to_string(),
        identity,
        bridge_profile,
        usage,
    })
}

fn profile_identity(
    snapshot: &AppStateSnapshot,
    whoami: Option<&crate::gateway::types::WhoamiResponse>,
) -> ProfileIdentity {
    let id = snapshot.verified_identity.as_ref();
    ProfileIdentity {
        email: whoami
            .and_then(|w| w.email.clone())
            .or_else(|| id.and_then(|i| i.email.clone())),
        user_id: whoami
            .and_then(|w| w.user_id.clone())
            .or_else(|| id.and_then(|i| i.user_id.clone())),
        tenant_id: whoami
            .and_then(|w| w.tenant_id.clone())
            .or_else(|| id.and_then(|i| i.tenant_id.clone())),
        display_name: whoami.and_then(|w| w.display_name.clone()),
        provider: whoami.and_then(|w| w.provider.clone()),
        roles: whoami.map(|w| w.roles.clone()).unwrap_or_default(),
        exp_unix: id.and_then(|i| i.exp_unix),
        verified_at_unix: id.map(|i| i.verified_at_unix),
        token_length: snapshot.cached_token.as_ref().map(|t| t.length),
        token_ttl_seconds: snapshot.cached_token.as_ref().map(|t| t.ttl_seconds),
        extra: whoami.map(|w| w.extra.clone()).unwrap_or_default(),
    }
}
