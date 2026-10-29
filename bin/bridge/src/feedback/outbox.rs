//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{DeviceId, UserId};
use systemprompt_models::feedback::receipts::{ConsumerReceiptRequest, ConsumerReceiptResponse};

#[path = "outbox_delivery.rs"]
mod delivery_impl;
#[path = "outbox_installations.rs"]
mod installations_impl;
#[path = "outbox_sessions.rs"]
mod sessions_impl;
use delivery_impl::refused_anywhere;
pub use installations_impl::PendingInstallation;

const MAX_ENTRIES: usize = 512;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Unacknowledged,
    Acknowledged(ConsumerReceiptResponse),
    Conflict,
    CredentialRejected,
    Rejected(u16),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub request: ConsumerReceiptRequest,
    pub delivery: Delivery,
    pub attempts: u32,
    pub next_attempt: DateTime<Utc>,
    pub session_bindings: BTreeMap<String, bool>,
    #[serde(default)]
    pub refused_sessions: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxScope {
    pub gateway: super::credentials::GatewayOrigin,
    pub consumer_id: UserId,
    pub device_id: DeviceId,
}

impl OutboxScope {
    pub fn from_enrollment(enrollment: &super::credentials::Enrollment) -> Self {
        Self {
            gateway: enrollment.gateway.clone(),
            consumer_id: enrollment.consumer_id.clone(),
            device_id: enrollment.device_id.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct State {
    scope: OutboxScope,
    entries: BTreeMap<String, Entry>,
    #[serde(default)]
    pending_installations: BTreeMap<String, PendingInstallation>,
    #[serde(default)]
    sessions: BTreeMap<String, Vec<systemprompt_identifiers::PublicationId>>,
    #[serde(default)]
    completed_sessions: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct Outbox {
    path: PathBuf,
    scope: OutboxScope,
}

impl Outbox {
    pub const fn new(path: PathBuf, scope: OutboxScope) -> Self {
        Self { path, scope }
    }

    fn lock_file(&self) -> Result<std::fs::File> {
        let parent = self.path.parent().ok_or(FeedbackError::Scope)?;
        crate::fsutil::create_dir_all_mode_0700(parent)?;
        let lock_path = self.path.with_extension("lock");
        Ok(std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?)
    }

    fn read_locked<T>(&self, inspect: impl FnOnce(&State) -> Result<T>) -> Result<T> {
        let lock = self.lock_file()?;
        lock.lock_shared()?;
        let state = read(&self.path, &self.scope)?;
        inspect(&state)
    }

    fn mutate<T>(&self, apply: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
        let lock = self.lock_file()?;
        lock.lock()?;
        let mut state = read(&self.path, &self.scope)?;
        let before = serde_json::to_vec(&state)?;
        let result = apply(&mut state)?;
        let bytes = serde_json::to_vec(&state)?;
        if bytes == before && self.path.exists() {
            return Ok(result);
        }
        if bytes.len() > MAX_BYTES {
            return Err(FeedbackError::Full);
        }
        crate::fsutil::atomic_write_0600(&self.path, &bytes)?;
        Ok(result)
    }

    pub fn require_enrollment(&self, enrollment: &super::credentials::Enrollment) -> Result<()> {
        if self.scope != OutboxScope::from_enrollment(enrollment) {
            return Err(FeedbackError::Scope);
        }
        Ok(())
    }

    pub fn enqueue(&self, mut request: ConsumerReceiptRequest) -> Result<String> {
        request.validate()?;
        request
            .files
            .sort_by(|a, b| (&a.revision_id, &a.path).cmp(&(&b.revision_id, &b.path)));
        request.runtime_files.sort_by(|a, b| a.path.cmp(&b.path));
        let key = key(&request)?;
        self.mutate(|state| {
            if let Some(existing) = state.entries.get_mut(&key) {
                let observed_at = request.observed_at;
                request.observed_at = existing.request.observed_at;
                if existing.request == request {
                    return Ok(key);
                }
                // Why: only acknowledged evidence is immutable. An entry the
                // gateway never accepted (pending, conflicted, rejected) is a
                // stale observation, and pinning it made every later readback
                // of the same publication fail on every sync, forever.
                if matches!(existing.delivery, Delivery::Acknowledged(_)) {
                    return Err(FeedbackError::Readback(
                        super::ReadbackFault::AcknowledgedConflict,
                    ));
                }
                request.observed_at = observed_at;
                existing.request = request;
                // Why: a re-observation is not a delivery; keeping `attempts`
                // keeps the backoff, so a receipt re-read every sync cannot
                // reset itself into a hot retry loop.
                existing.delivery = Delivery::Unacknowledged;
                return Ok(key);
            }
            if state.entries.len() >= MAX_ENTRIES {
                let removable = state
                    .entries
                    .iter()
                    .filter(|(_, entry)| {
                        matches!(entry.delivery, Delivery::Rejected(_))
                            || matches!(entry.delivery, Delivery::Acknowledged(_))
                                && entry.session_bindings.values().all(|done| *done)
                                && !state.sessions.iter().any(|(key, publications)| {
                                    !state.completed_sessions.contains(key)
                                        && publications.contains(&entry.request.publication_id)
                                })
                    })
                    .min_by_key(|(_, entry)| entry.request.observed_at)
                    .map(|(key, _)| key.clone());
                if let Some(removable) = removable {
                    state.entries.remove(&removable);
                } else {
                    return Err(FeedbackError::Full);
                }
            }
            let mut session_bindings = BTreeMap::new();
            for (session_key, publications) in &state.sessions {
                if state.completed_sessions.contains(session_key)
                    || !publications.contains(&request.publication_id)
                {
                    continue;
                }
                let (host, session): (systemprompt_models::feedback::EvaluatorClient, String) =
                    serde_json::from_str(session_key)?;
                if host == request.host && !refused_anywhere(state, host, &session) {
                    session_bindings.insert(session, false);
                }
            }
            state.entries.insert(
                key.clone(),
                Entry {
                    request,
                    delivery: Delivery::Unacknowledged,
                    attempts: 0,
                    next_attempt: Utc::now(),
                    session_bindings,
                    refused_sessions: BTreeMap::new(),
                },
            );
            Ok(key)
        })
    }

    pub fn entries(&self) -> Result<Vec<(String, Entry)>> {
        self.read_locked(|state| {
            Ok(state
                .entries
                .iter()
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect())
        })
    }
}

pub(super) fn retry_after(attempts: u32) -> chrono::Duration {
    chrono::Duration::seconds(2i64.pow(attempts.min(10)).min(3600))
}

fn key(request: &ConsumerReceiptRequest) -> Result<String> {
    Ok(crate::hash::sha256_hex(&serde_json::to_vec(&(
        &request.installation_id,
        &request.resource_id,
        &request.publication_id,
        request.host,
    ))?))
}

fn read(path: &Path, scope: &OutboxScope) -> Result<State> {
    if !path.exists() {
        return Ok(State {
            scope: scope.clone(),
            entries: BTreeMap::new(),
            pending_installations: BTreeMap::new(),
            sessions: BTreeMap::new(),
            completed_sessions: BTreeSet::new(),
        });
    }
    if std::fs::metadata(path)?.len() > MAX_BYTES as u64 {
        return Err(FeedbackError::Full);
    }
    let state: State = serde_json::from_slice(&std::fs::read(path)?)?;
    if &state.scope != scope {
        return Err(FeedbackError::Scope);
    }
    Ok(state)
}
