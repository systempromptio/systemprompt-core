//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Result;
use super::credentials::Enrollment;
use super::outbox::Outbox;
use systemprompt_identifiers::{ClientSessionId, NativeSessionId};
use systemprompt_models::feedback::EvaluatorClient;

/// Header the `OpenCode` plugin stamps on every chat request with the
/// gateway-facing session UUID.
pub const OPENCODE_SESSION_HEADER: &str = "x-opencode-session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSession {
    pub host: EvaluatorClient,
    pub id: NativeSessionId,
}

pub fn native_session(headers: &http::HeaderMap, body: &[u8]) -> Option<NativeSession> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let user_agent = headers
        .get(http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let host = if user_agent.contains("hermes") {
        EvaluatorClient::Hermes
    } else if user_agent.contains("opencode") {
        EvaluatorClient::OpenCode
    } else if user_agent.contains("codex")
        || value
            .pointer("/client_metadata/x-codex-turn-metadata")
            .is_some()
    {
        EvaluatorClient::Codex
    } else if user_agent.contains("claude-desktop") {
        EvaluatorClient::ClaudeDesktop
    } else if user_agent.contains("claude-cli") || user_agent.contains("claude-code") {
        EvaluatorClient::ClaudeCode
    } else {
        return None;
    };
    let session = match host {
        EvaluatorClient::ClaudeCode | EvaluatorClient::ClaudeDesktop => {
            let metadata = value.pointer("/metadata/user_id")?.as_str()?;
            ClientSessionId::from_metadata_user_id(metadata)
                .ok()??
                .as_str()
                .to_owned()
        },
        EvaluatorClient::Codex => {
            if let Some(encoded) = value
                .pointer("/client_metadata/x-codex-turn-metadata")
                .and_then(serde_json::Value::as_str)
            {
                let metadata: serde_json::Value = serde_json::from_str(encoded).ok()?;
                metadata.get("thread_id")?.as_str()?.to_owned()
            } else if let Some(thread) = value
                .pointer("/client_metadata/thread_id")
                .and_then(serde_json::Value::as_str)
            {
                thread.to_owned()
            } else {
                let dash = headers
                    .get("session-id")
                    .and_then(|value| value.to_str().ok());
                let underscore = headers
                    .get("session_id")
                    .and_then(|value| value.to_str().ok());
                if dash.is_some() && underscore.is_some() && dash != underscore {
                    return None;
                }
                dash.or(underscore)?.to_owned()
            }
        },
        EvaluatorClient::OpenCode => {
            let raw = headers.get(OPENCODE_SESSION_HEADER)?.to_str().ok()?;
            // Why: the current plugin sends the v5 UUID; a plugin from before
            // the mapping sends the raw `ses_…` id, which maps to the same UUID.
            match ClientSessionId::try_new(raw) {
                Ok(id) => id.as_str().to_owned(),
                Err(_) => super::opencode_session::session_uuid(raw)
                    .ok()?
                    .as_str()
                    .to_owned(),
            }
        },
        EvaluatorClient::Hermes => value.get("session_id")?.as_str()?.to_owned(),
    };
    if session.is_empty() || session.len() > 512 || session.chars().any(char::is_control) {
        return None;
    }
    Some(NativeSession {
        host,
        id: NativeSessionId::new(session),
    })
}

pub fn observe(gateway: &str, headers: &http::HeaderMap, body: &[u8]) -> Result<()> {
    let Some(session) = native_session(headers, body) else {
        return Ok(());
    };
    let root = super::metadata_root()?;
    let enrollment = Enrollment::load(&root, gateway)?;
    Outbox::new(
        enrollment.outbox_path(&root),
        crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
    )
    .queue_session(session.host, session.id.as_str())
}
