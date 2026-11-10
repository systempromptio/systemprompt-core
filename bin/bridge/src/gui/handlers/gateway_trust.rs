//! GUI handlers for the first trust of a gateway's signing key: show the
//! fingerprint the gateway serves, and pin it only once the user confirms
//! that fingerprint for that gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::{GuiApp, emit};
use crate::gui_policy::trust::{self, key_fingerprint};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

#[derive(Debug, Clone)]
pub enum TrustOp {
    Inspect,
    Confirm {
        gateway: String,
        fingerprint: String,
    },
}

pub(crate) fn on_gateway_trust_requested(app: &GuiApp, op: TrustOp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let confirming = matches!(op, TrustOp::Confirm { .. });
        let result = run(op, http).await;
        let pinned = confirming && result.is_ok();
        proxy.send_event(UiEvent::GatewayTrustFinished {
            result,
            pinned,
            reply_to,
        });
    });
}

pub(crate) fn on_gateway_trust_finished(
    app: &mut GuiApp,
    // JSON: webview IPC reply for the trust inspect/confirm command
    result: Result<Value, BridgeError>,
    pinned: bool,
    reply_to: ReplyId,
) {
    if pinned {
        app.append_log("Gateway signing key trusted after fingerprint confirmation");
        app.state.reload();
        app.refresh_ui();
        app.proxy
            .send_event(UiEvent::SyncRequested { reply_to: None });
    }
    emit::finish(app, reply_to, result);
}

fn err(code: ErrorCode, error: &dyn std::error::Error) -> BridgeError {
    BridgeError::from_error(ErrorScope::Gateway, code, error)
}

// JSON: webview IPC reply for the trust inspect/confirm command
async fn run(op: TrustOp, http: reqwest::Client) -> Result<Value, BridgeError> {
    let cfg = config::load().map_err(|e| err(ErrorCode::Internal, &e))?;
    let gateway = config::gateway_url_or_default(&cfg);
    let state = config::trust::pinned_pubkey_state_for(&cfg, &gateway)
        .map_err(|e| err(ErrorCode::Internal, &e))?;
    if !matches!(state, config::PinnedPubkeyState::Unpinned) {
        return Err(BridgeError::new(
            ErrorScope::Gateway,
            ErrorCode::Conflict,
            "this gateway already has a pinned signing key",
        ));
    }
    let served = GatewayClient::new(gateway.clone(), http)
        .fetch_pubkey()
        .await
        .map_err(|e| err(ErrorCode::Unreachable, &e))?;
    match op {
        TrustOp::Inspect => Ok(json!({
            "gateway": gateway.as_str(),
            "fingerprint": key_fingerprint(&served),
        })),
        TrustOp::Confirm {
            gateway: confirmed_gateway,
            fingerprint,
        } => {
            let key = trust::confirm(&confirmed_gateway, &fingerprint, &gateway, &served)
                .map_err(|e| err(ErrorCode::Conflict, &e))?;
            config::persist_pinned_pubkey(&gateway, key)
                .map_err(|e| err(ErrorCode::Internal, &e))?;
            Ok(json!({ "gateway": gateway.as_str(), "fingerprint": key_fingerprint(key) }))
        },
    }
}
