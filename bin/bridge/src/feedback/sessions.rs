//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials::Enrollment;
use super::outbox::Outbox;
use super::{FeedbackError, Result};
use parking_lot::Mutex;
use std::collections::BTreeSet;
use systemprompt_identifiers::{ClientSessionId, NativeSessionId};
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::wire::origin::{ClientKind, native_marker, ua_product};

use crate::ids::HostId;

pub const OPENCODE_SESSION_HEADER: &str = "x-opencode-session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSession {
    pub host: EvaluatorClient,
    pub id: NativeSessionId,
}

// Why: the host a request is bound to must be the host the gateway records
// for it, so the same evidence decides both: the verified host token first,
// then the shared native-marker and User-Agent helpers.
pub fn native_session(
    verified_host: Option<&HostId>,
    headers: &http::HeaderMap,
    body: &[u8],
) -> Option<NativeSession> {
    // JSON: protocol boundary — the inference body is any host's wire shape.
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let host = EvaluatorClient::try_from(presenting_client(verified_host, headers, body)?).ok()?;
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
                // JSON: protocol boundary — Codex turn metadata is an opaque JSON string.
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

fn presenting_client(
    verified_host: Option<&HostId>,
    headers: &http::HeaderMap,
    body: &[u8],
) -> Option<ClientKind> {
    if let Some(host) = verified_host {
        return ClientKind::from_bridge_host_id(host.as_str());
    }
    if let Some(marker) = native_marker(body) {
        return Some(marker.client());
    }
    let user_agent = headers
        .get(http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let (product, _) = ua_product(user_agent)?;
    ClientKind::from_ua_product(&product)
}

/// Native sessions seen on the request path and not yet recorded in the outbox.
///
/// Observation is an in-memory insert; the outbox write happens when the
/// heartbeat task calls [`NativeSessionLedger::flush`], so the proxy never
/// carries a file transaction. Past `MAX_UNFLUSHED` a new session is refused.
#[derive(Debug, Default)]
pub struct NativeSessionLedger {
    unflushed: Mutex<BTreeSet<(EvaluatorClient, NativeSessionId)>>,
}

const MAX_UNFLUSHED: usize = 1024;

impl NativeSessionLedger {
    pub fn observe(
        &self,
        verified_host: Option<&HostId>,
        headers: &http::HeaderMap,
        body: &[u8],
    ) -> Result<()> {
        let Some(session) = native_session(verified_host, headers, body) else {
            return Ok(());
        };
        let key = (session.host, session.id);
        let mut unflushed = self.unflushed.lock();
        if unflushed.contains(&key) {
            return Ok(());
        }
        if unflushed.len() >= MAX_UNFLUSHED {
            return Err(FeedbackError::Full);
        }
        unflushed.insert(key);
        drop(unflushed);
        Ok(())
    }

    pub fn unflushed(&self) -> usize {
        self.unflushed.lock().len()
    }

    pub fn flush(&self, gateway: &str) -> Result<()> {
        let pending = std::mem::take(&mut *self.unflushed.lock());
        if pending.is_empty() {
            return Ok(());
        }
        let outbox = match open_outbox(gateway) {
            Ok(outbox) => outbox,
            Err(error) => {
                self.unflushed.lock().extend(pending);
                return Err(error);
            },
        };
        let mut failure = None;
        for (host, session) in pending {
            if let Err(error) = outbox.queue_session(host, session.as_str()) {
                self.unflushed.lock().insert((host, session));
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

fn open_outbox(gateway: &str) -> Result<Outbox> {
    let root = super::metadata_root()?;
    let enrollment = Enrollment::load(&root, gateway)?;
    Ok(Outbox::new(
        enrollment.outbox_path(&root),
        crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
    ))
}
