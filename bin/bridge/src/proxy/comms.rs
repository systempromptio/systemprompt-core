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

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use systemprompt_identifiers::ValidatedUrl;

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
struct CommsAnnouncement {
    #[serde(rename = "messageId")]
    message_id: crate::ids::CommsMessageId,
    #[serde(rename = "sessionId")]
    session_id: Option<crate::ids::HookSessionId>,
    from: String,
    #[serde(rename = "deliveryClass")]
    delivery_class: String,
    preview: String,
}

#[must_use]
pub fn inbox_dir() -> Option<PathBuf> {
    crate::basedirs::config_dir().map(|d| d.join("inbox"))
}

// Why: a per-session filename rather than a per-user one: the hook that reads
// this knows only its own session id, and the isolation guarantee has to hold
// even if the hook script is wrong. A message it must not see is not in a file
// it can name.
fn inbox_path(session_id: &crate::ids::HookSessionId) -> Option<PathBuf> {
    let safe: String = session_id
        .as_str()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        return None;
    }
    inbox_dir().map(|d| d.join(format!("{safe}.jsonl")))
}

fn append(announcement: &CommsAnnouncement) {
    // Why: An announcement with no session is inbox-class and must not be written:
    // it would surface in whichever session happened to read first, which is
    // exactly the interruption the delivery classes prevent.
    let Some(session_id) = announcement.session_id.as_ref() else {
        return;
    };
    let Some(path) = inbox_path(session_id) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(parent) {
        tracing::warn!(error = %e, "could not create the comms inbox directory");
        return;
    }
    let Ok(line) = serde_json::to_string(announcement) else {
        return;
    };
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(e) = writeln!(file, "{line}") {
                tracing::warn!(error = %e, "could not append to the comms inbox");
            }
        },
        Err(e) => tracing::warn!(error = %e, "could not open the comms inbox"),
    }
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
        append(&announcement);
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
        match subscribe_once(cfg.gateway_base.as_ref(), token_cache.as_ref(), &client).await {
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
