use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::ipc_runtime;
use crate::gui::state::{
    GatewayProbeOutcome, GatewayStatus, decode_jwt_identity_unverified, now_unix,
};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_gateway_probe_requested(app: &mut GuiApp, reply_to: ReplyId) {
    app.state.mark_probing();
    app.refresh_ui();
    ipc_runtime::emit_gateway_changed(app);
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
    app.state.apply_probe(outcome);
    app.refresh_ui();
    ipc_runtime::emit_gateway_changed(app);

    let Some(id) = reply_to else {
        if let Err(err) = bridge_result {
            ipc_runtime::emit_error(app, &err);
        }
        return;
    };
    let payload = match bridge_result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => IpcReplyPayload::err(err),
    };
    ipc_runtime::send_reply_payload(app, id, &payload);
}

pub(crate) fn spawn_probe(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let outcome = tokio::task::spawn_blocking(|| {
            let cfg = config::load();
            let gateway = config::gateway_url_or_default(&cfg);
            let client = GatewayClient::new(gateway);

            let started = std::time::Instant::now();
            let status = match client.health() {
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
                obtain_live_token(&cfg).and_then(|tok| decode_jwt_identity_unverified(tok.expose()))
            } else {
                if !crate::auth::has_credential_source(&cfg) {
                    let _ = crate::auth::cache::clear();
                }
                None
            };

            GatewayProbeOutcome {
                status,
                identity,
                at_unix: now_unix(),
            }
        })
        .await
        .unwrap_or_else(|_| GatewayProbeOutcome {
            status: GatewayStatus::Unreachable {
                reason: "probe task panicked".into(),
            },
            identity: None,
            at_unix: now_unix(),
        });
        let _ = proxy.send_event(UiEvent::GatewayProbeFinished { outcome, reply_to });
    });
}

fn obtain_live_token(cfg: &config::Config) -> Option<crate::auth::secret::Secret> {
    crate::auth::obtain_live_token(cfg).map(|out| out.token)
}
