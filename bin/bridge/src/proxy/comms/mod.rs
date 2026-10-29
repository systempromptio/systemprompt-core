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
use systemprompt_identifiers::{UserId, ValidatedUrl};

mod inbox;

use inbox::append;
pub use inbox::{
    DRAINING_SUFFIX, INBOX_DIR_NAME, InboxError, inbox_dir, inbox_path, sweep_draining,
};

use crate::config::SharedRuntimeConfig;
use crate::proxy::drive_signal::{DriveSignal, DriveSignals, notice_for};
use crate::proxy::token_cache::TokenCache;
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::{DocumentPresenceEvent, DocumentRevisionCreatedEvent};

const AUTH_THRESHOLD_SECS: u64 = 300;
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);
const EVENT_NAME: &str = "comms.message";

#[derive(Debug, Deserialize)]
struct AgUiEnvelope {
    #[serde(default)]
    name: Option<String>,
    // JSON: the value's shape is chosen by `name`; each arm decodes its own type.
    #[serde(default)]
    value: Option<Box<serde_json::value::RawValue>>,
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

fn handle_frame(data: &str, drive: &DriveSignals, viewer: Option<&UserId>) {
    let Ok(envelope) = serde_json::from_str::<AgUiEnvelope>(data) else {
        return;
    };
    let (Some(name), Some(value)) = (envelope.name.as_deref(), envelope.value) else {
        return;
    };
    match name {
        EVENT_NAME => {
            if let Some(announcement) = decode(&value) {
                deliver_announcement(&announcement);
            }
        },
        DocumentRevisionCreatedEvent::NAME => {
            if let Some(event) = decode(&value) {
                drive.send(DriveSignal::RevisionCreated(Box::new(event)));
            }
        },
        DocumentPresenceEvent::NAME => {
            if let Some(event) = decode(&value) {
                drive.send(DriveSignal::Presence(Box::new(event)));
            }
        },
        other => {
            if let Some(notice) = notice_for(other, value.get(), viewer) {
                drive.send(DriveSignal::Notice(Box::new(notice)));
            }
        },
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: &serde_json::value::RawValue) -> Option<T> {
    match serde_json::from_str(value.get()) {
        Ok(decoded) => Some(decoded),
        Err(error) => {
            tracing::warn!(%error, "bridge stream frame did not decode; it is skipped");
            None
        },
    }
}

fn deliver_announcement(announcement: &CommsAnnouncement) {
    tracing::debug!(
        message_id = %announcement.message_id,
        class = %announcement.delivery_class,
        "comms announcement received"
    );
    if let Err(e) = append(announcement) {
        tracing::warn!(
            message_id = %announcement.message_id,
            error = %e,
            "comms announcement dropped: it could not be written to the inbox"
        );
    }
}

pub async fn run_loop(
    runtime_config: SharedRuntimeConfig,
    token_cache: Arc<TokenCache>,
    client: reqwest::Client,
    drive: DriveSignals,
) {
    let mut backoff = RETRY_MIN;
    loop {
        let cfg = runtime_config.load_full();
        let mut generation = token_cache.generation();
        generation.mark_unchanged();
        let outcome = tokio::select! {
            outcome = subscribe_once(cfg.gateway_base.as_ref(), token_cache.as_ref(), &client, &drive) => outcome,
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
    #[error("gateway answered {status}: {rejection}")]
    Status {
        status: reqwest::StatusCode,
        rejection: Box<crate::gateway::GatewayRejection>,
    },
}

async fn subscribe_once(
    gateway_base: &ValidatedUrl,
    token_cache: &TokenCache,
    client: &reqwest::Client,
    drive: &DriveSignals,
) -> Result<(), CommsError> {
    let token = token_cache.current(AUTH_THRESHOLD_SECS).await?;

    let url = format!(
        "{base}{path}",
        base = gateway_base.as_str().trim_end_matches('/'),
        path = ApiPaths::BRIDGE_STREAM,
    );

    let response = client
        .get(&url)
        .bearer_auth(token.token.expose())
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        token_cache.reject_upstream(ApiPaths::BRIDGE_STREAM).await;
        return Err(CommsError::Unauthorized);
    }
    if !response.status().is_success() {
        let (status, rejection) = crate::gateway::GatewayRejection::read(response).await;
        return Err(CommsError::Status { status, rejection });
    }

    let viewer = crate::auth::jwt::decode_unverified(token.token.expose())
        .and_then(|identity| identity.user_id);
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
                    handle_frame(data, drive, viewer.as_ref());
                }
            }
        }
    }
    Ok(())
}
