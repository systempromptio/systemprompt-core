//! GUI handlers probing gateway reachability and auth state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::notify::Signal;
use crate::gui::state::{
    CancelScope, GatewayProbeOutcome, GatewayStatus, decode_jwt_identity_unverified, now_unix,
};
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_gateway_probe_requested(app: &mut GuiApp, reply_to: ReplyId) {
    if app.state.gateway_probe_in_flight() {
        if let Some(id) = reply_to {
            emit::send_reply(app, id, json!({ "inFlight": true }), true);
        }
        return;
    }
    app.state.mark_probing();
    app.refresh_ui();
    emit::emit_gateway_changed(app);
    spawn_probe(app, reply_to);
}

pub(crate) fn on_gateway_probe_finished(
    app: &mut GuiApp,
    outcome: Option<GatewayProbeOutcome>,
    reply_to: ReplyId,
) {
    let Some(outcome) = outcome else {
        app.state.clear_cancel(CancelScope::GatewayProbe);
        app.state.abandon_probe();
        app.refresh_ui();
        emit::emit_gateway_changed(app);
        if let Some(id) = reply_to {
            emit::send_reply(app, id, json!({ "state": "cancelled" }), true);
        }
        return;
    };
    let bridge_result = match &outcome.status {
        GatewayStatus::Reachable { latency_ms } => Ok(json!({
            "state": "reachable",
            "latencyMs": latency_ms,
            "identity": outcome.identity.as_ref().map(|i| json!({
                "email": i.email,
                "user_id": i.user_id,
                "tenant_id": i.tenant_id,
                "exp_unix": i.exp_unix,
            })),
        })),
        GatewayStatus::Unreachable { reason } => Err(BridgeError::new(
            ErrorScope::Gateway,
            ErrorCode::Unreachable,
            reason.clone(),
        )),
        GatewayStatus::Probing => Ok(json!({ "state": "probing" })),
        GatewayStatus::Unknown => Ok(json!({ "state": "unknown" })),
    };
    app.state.clear_cancel(CancelScope::GatewayProbe);
    app.state.apply_probe(outcome);
    app.refresh_ui();
    announce(app);
    emit::emit_gateway_changed(app);
    emit::emit_state(app);

    let Some(id) = reply_to else {
        if let Err(err) = bridge_result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match bridge_result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => IpcReplyPayload::err(err),
    };
    emit::send_reply_payload(app, id, &payload);
}

const SESSION_EXPIRY_WARN_SECS: u64 = 24 * 60 * 60;

fn announce(app: &mut GuiApp) {
    let snap = app.state.snapshot();
    let app_name = crate::brand::brand().app_name;
    match &snap.gateway_status {
        GatewayStatus::Unreachable { reason } => {
            let reason = reason.clone();
            app.signal_raised(
                Signal::GatewayUnreachable,
                &format!("{app_name} cannot reach the gateway"),
                &format!("Agent traffic is ungoverned until it comes back: {reason}"),
            );
        },
        _ => app.signal_cleared(Signal::GatewayUnreachable),
    }

    let expiring = snap
        .verified_identity
        .as_ref()
        .and_then(|id| id.exp_unix)
        .is_some_and(|exp| exp.saturating_sub(now_unix()) <= SESSION_EXPIRY_WARN_SECS);
    if expiring {
        app.signal_raised(
            Signal::SessionExpiring,
            &format!("{app_name} session expires soon"),
            "Sign in again from the account menu to keep syncing without interruption.",
        );
    } else {
        app.signal_cleared(Signal::SessionExpiring);
    }
}

pub(crate) fn spawn_probe(app: &GuiApp, reply_to: ReplyId) {
    if app.state.gateway_probe_in_flight() {
        return;
    }
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::GatewayProbe);
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let outcome = tokio::select! {
            () = token.cancelled() => None,
            outcome = run_probe(&http) => Some(outcome),
        };
        proxy.send_event(UiEvent::GatewayProbeFinished { outcome, reply_to });
    });
}

async fn run_probe(http: &reqwest::Client) -> GatewayProbeOutcome {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway, http.clone());

    let started = std::time::Instant::now();
    let status = match client.health().await {
        Ok(()) => GatewayStatus::Reachable {
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        },
        Err(e) => GatewayStatus::Unreachable {
            reason: e.to_string(),
        },
    };

    let identity = if matches!(status, GatewayStatus::Reachable { .. })
        && crate::auth::has_credential_source(&cfg)
    {
        obtain_live_token(&cfg, http)
            .await
            .and_then(|tok| decode_jwt_identity_unverified(tok.expose()))
    } else {
        if !crate::auth::has_credential_source(&cfg) {
            _ = crate::auth::cache::clear();
        }
        None
    };

    let provider_health = if matches!(status, GatewayStatus::Reachable { .. }) {
        client
            .fetch_bridge_profile()
            .await
            .map(|profile| profile.providers)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    GatewayProbeOutcome {
        status,
        identity,
        at_unix: now_unix(),
        provider_health,
    }
}

async fn obtain_live_token(
    cfg: &config::Config,
    http: &reqwest::Client,
) -> Option<crate::auth::secret::Secret> {
    crate::auth::obtain_live_token(cfg, &systemprompt_identifiers::SessionId::generate(), http)
        .await
        .map(|out| out.token)
}
