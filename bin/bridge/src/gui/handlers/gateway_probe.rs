use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::{
    CancelScope, GatewayProbeOutcome, GatewayStatus, decode_jwt_identity_unverified, now_unix,
};
use crate::gui::{GuiApp, emit, ipc_runtime};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_gateway_probe_requested(app: &mut GuiApp, reply_to: ReplyId) {
    app.state.mark_probing();
    app.refresh_ui();
    emit::emit_gateway_changed(app);
    spawn_probe(app, reply_to);
}

pub(crate) fn on_gateway_probe_finished(
    app: &mut GuiApp,
    outcome: GatewayProbeOutcome,
    reply_to: ReplyId,
) {
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
        GatewayStatus::Probing => Err(BridgeError::new(
            ErrorScope::Gateway,
            ErrorCode::Internal,
            "probe still in flight",
        )),
        GatewayStatus::Unknown => Err(BridgeError::new(
            ErrorScope::Gateway,
            ErrorCode::Internal,
            "probe outcome unknown",
        )),
    };
    app.state.clear_cancel(CancelScope::GatewayProbe);
    app.state.apply_probe(outcome);
    app.refresh_ui();
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

pub(crate) fn spawn_probe(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::GatewayProbe);
    app.runtime.spawn(async move {
        let outcome = tokio::select! {
            _ = token.cancelled() => GatewayProbeOutcome {
                status: GatewayStatus::Unreachable {
                    reason: "probe cancelled".into(),
                },
                identity: None,
                at_unix: now_unix(),
            },
            outcome = run_probe() => outcome,
        };
        // Why: best-effort UI notification — once the event loop shuts down the
        // receiver is dropped and there is nothing left to deliver the outcome to.
        _ = proxy.send_event(UiEvent::GatewayProbeFinished { outcome, reply_to });
    });
}

async fn run_probe() -> GatewayProbeOutcome {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway);

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
        obtain_live_token(&cfg)
            .await
            .and_then(|tok| decode_jwt_identity_unverified(tok.expose()))
    } else {
        if !crate::auth::has_credential_source(&cfg) {
            // Why: best-effort eviction of a now-orphaned token cache; absence is
            // the desired post-condition, so a failed clear is not actionable.
            _ = crate::auth::cache::clear();
        }
        None
    };

    GatewayProbeOutcome {
        status,
        identity,
        at_unix: now_unix(),
    }
}

async fn obtain_live_token(cfg: &config::Config) -> Option<crate::auth::secret::Secret> {
    crate::auth::obtain_live_token(cfg, &systemprompt_identifiers::SessionId::generate())
        .await
        .map(|out| out.token)
}
