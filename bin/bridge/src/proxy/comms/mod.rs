//! Team-comms inbox delivery: a long-lived SSE subscription to
//! `GET /v1/bridge/stream`, writing each announcement to a per-session inbox
//! file the client hooks read.
//!
//! This lives in the proxy rather than the sync agent because the sync agent
//! is a scheduled task — it exists only for the seconds it takes to apply a
//! manifest. The proxy is the one bridge component that runs continuously, so
//! it is the only place a subscription can be held.
//!
//! One file per session, never one shared file. A hook reads only the file
//! named for the session it is running in, so a message addressed elsewhere is
//! not merely filtered out — it was never written where the wrong hook could
//! find it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use systemprompt_identifiers::ValidatedUrl;

mod inbox;

use inbox::append;
pub use inbox::{
    DRAINING_SUFFIX, INBOX_DIR_NAME, InboxError, inbox_dir, inbox_path, sweep_draining,
};

use crate::config::SharedRuntimeConfig;
use crate::proxy::token_cache::TokenCache;

const AUTH_THRESHOLD_SECS: u64 = 300;
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
const EVENT_NAME: &str = "comms.message";

#[derive(Debug, Deserialize)]
struct AgUiEnvelope {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<CommsAnnouncement>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub(super) struct CommsAnnouncement {
    #[serde(rename = "messageId")]
    message_id: crate::ids::CommsMessageId,
    #[serde(rename = "sessionId")]
    session_id: Option<crate::ids::HookSessionId>,
    from: String,
    #[serde(rename = "deliveryClass")]
    delivery_class: String,
    preview: String,
}

fn handle_frame(data: &str) {
    let Ok(envelope) = serde_json::from_str::<AgUiEnvelope>(data) else {
        return;
    };
    if envelope.name.as_deref() != Some(EVENT_NAME) {
        return;
    }
    if let Some(announcement) = envelope.value {
        tracing::debug!(
            message_id = %announcement.message_id,
            class = %announcement.delivery_class,
            "comms announcement received"
        );
        if let Err(e) = append(&announcement) {
            tracing::warn!(
                message_id = %announcement.message_id,
                error = %e,
                "comms announcement dropped: it could not be written to the inbox"
            );
        }
    }
}

pub async fn run_loop(
    runtime_config: SharedRuntimeConfig,
    token_cache: Arc<TokenCache>,
    client: reqwest::Client,
) {
    let mut backoff = RETRY_MIN;
    loop {
        let cfg = runtime_config.load_full();
        let mut generation = token_cache.generation();
        generation.mark_unchanged();
        let outcome = tokio::select! {
            outcome = subscribe_once(cfg.gateway_base.as_ref(), token_cache.as_ref(), &client) => outcome,
            // Why: the stream belongs to the gateway it was opened against; a
            // swap means that gateway is no longer this bridge's, however
            // healthy the connection still is.
            _ = generation.changed() => {
                tracing::info!(gateway = %cfg.gateway_base, "comms stream dropped: runtime config swapped");
                backoff = RETRY_MIN;
                continue;
            }
        };
        match outcome {
            Ok(()) => {
                tracing::info!("comms stream closed by the gateway; reconnecting");
                backoff = RETRY_MIN;
            },
            Err(e) => {
                tracing::warn!(error = %e, retry_in = ?backoff, "comms stream failed");
                backoff = (backoff * 2).min(RETRY_MAX);
            },
        }
        tokio::time::sleep(backoff).await;
    }
}

#[derive(Debug, thiserror::Error)]
enum CommsError {
    #[error("token: {0}")]
    Token(#[from] crate::proxy::forward::ForwardError),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("gateway rejected the comms subscription")]
    Unauthorized,
    #[error("gateway answered {0}")]
    Status(reqwest::StatusCode),
}

async fn subscribe_once(
    gateway_base: &ValidatedUrl,
    token_cache: &TokenCache,
    client: &reqwest::Client,
) -> Result<(), CommsError> {
    let token = token_cache.current(AUTH_THRESHOLD_SECS).await?;

    let url = format!(
        "{base}/v1/bridge/stream",
        base = gateway_base.as_str().trim_end_matches('/'),
    );

    let response = client
        .get(&url)
        .bearer_auth(token.token.expose())
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        token_cache.reject_upstream("/v1/bridge/stream").await;
        return Err(CommsError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(CommsError::Status(response.status()));
    }

    tracing::info!("comms stream open");
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(idx) = buffer.find("\n\n") {
            let frame = buffer[..idx].to_owned();
            buffer.drain(..idx + 2);
            for line in frame.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    handle_frame(data);
                }
            }
        }
    }
    Ok(())
}
