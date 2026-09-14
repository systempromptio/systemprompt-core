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
use std::path::{Path, PathBuf};
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

pub const INBOX_DIR_NAME: &str = "inbox";
pub const DRAINING_SUFFIX: &str = ".draining";

#[must_use]
pub fn inbox_dir() -> Option<PathBuf> {
    crate::basedirs::config_dir().map(|d| {
        d.join(crate::brand::brand().config_dir)
            .join(INBOX_DIR_NAME)
    })
}

#[must_use]
pub fn inbox_path(session_id: &crate::ids::HookSessionId) -> Option<PathBuf> {
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

#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("announcement {message_id} names no session")]
    NoSession {
        message_id: crate::ids::CommsMessageId,
    },
    #[error("announcement {message_id} has an unusable session id")]
    UnusableSession {
        message_id: crate::ids::CommsMessageId,
    },
    #[error("no config directory for the comms inbox")]
    NoConfigDir,
    #[error("serialise announcement {message_id}: {source}")]
    Serialize {
        message_id: crate::ids::CommsMessageId,
        #[source]
        source: serde_json::Error,
    },
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// Why: a crash between renaming the inbox to `.draining` and reading it back
// would otherwise strand the messages; on start every leftover is folded
// back into the live inbox before any new announcement lands.
pub fn sweep_draining() -> Result<usize, InboxError> {
    let dir = inbox_dir().ok_or(InboxError::NoConfigDir)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(InboxError::Io {
                action: "enumerate",
                path: dir,
                source,
            });
        },
    };
    let mut restored = 0;
    for entry in entries {
        let entry = entry.map_err(|source| InboxError::Io {
            action: "enumerate",
            path: dir.clone(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(DRAINING_SUFFIX) else {
            continue;
        };
        let Some(session_stem) = stem.split_once(".jsonl.").map(|(s, _)| s) else {
            continue;
        };
        let target = dir.join(format!("{session_stem}.jsonl"));
        let body = std::fs::read(entry.path()).map_err(|source| InboxError::Io {
            action: "read",
            path: entry.path(),
            source,
        })?;
        append_bytes(&target, &body)?;
        std::fs::remove_file(entry.path()).map_err(|source| InboxError::Io {
            action: "remove",
            path: entry.path(),
            source,
        })?;
        restored += 1;
    }
    Ok(restored)
}

fn append_bytes(path: &Path, bytes: &[u8]) -> Result<(), InboxError> {
    let parent = path.parent().ok_or(InboxError::NoConfigDir)?;
    crate::fsutil::create_dir_all_mode_0700(parent).map_err(|source| InboxError::Io {
        action: "create",
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file = crate::fsutil::open_append_0600(path).map_err(|source| InboxError::Io {
        action: "open",
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes).map_err(|source| InboxError::Io {
        action: "append",
        path: path.to_path_buf(),
        source,
    })
}

fn append(announcement: &CommsAnnouncement) -> Result<(), InboxError> {
    let message_id = announcement.message_id.clone();
    let session_id = announcement
        .session_id
        .as_ref()
        .ok_or_else(|| InboxError::NoSession {
            message_id: message_id.clone(),
        })?;
    let path = inbox_path(session_id).ok_or_else(|| InboxError::UnusableSession {
        message_id: message_id.clone(),
    })?;
    let mut line = serde_json::to_vec(announcement)
        .map_err(|source| InboxError::Serialize { message_id, source })?;
    line.push(b'\n');
    append_bytes(&path, &line)
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
