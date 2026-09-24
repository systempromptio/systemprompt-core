//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::ReadbackFault;
use super::{FeedbackError, MAX_ENTRIES, Outbox, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use systemprompt_models::bridge::manifest::SkillPublication;
use systemprompt_models::feedback::EvaluatorClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInstallation {
    pub publication: SkillPublication,
    pub host: EvaluatorClient,
    pub roots: Vec<PathBuf>,
    pub superseded: bool,
    pub attempts: u32,
    pub next_attempt: DateTime<Utc>,
}

impl PendingInstallation {
    pub fn new(publication: SkillPublication, host: EvaluatorClient, roots: Vec<PathBuf>) -> Self {
        Self {
            publication,
            host,
            roots,
            superseded: false,
            attempts: 0,
            next_attempt: Utc::now(),
        }
    }
}

impl Outbox {
    pub fn reserve_installation(&self, pending: PendingInstallation) -> Result<String> {
        if pending.roots.is_empty() || pending.roots.len() > 256 {
            return Err(FeedbackError::Readback(ReadbackFault::NoRoots));
        }
        let key = crate::hash::sha256_hex(&serde_json::to_vec(&(
            &pending.publication.resource_id,
            &pending.publication.publication_id,
            pending.host,
        ))?);
        self.mutate(|state| {
            if let Some(existing) = state.pending_installations.get(&key) {
                if existing.roots != pending.roots
                    || existing.publication.bundle_digest != pending.publication.bundle_digest
                    || existing.publication.revision_id != pending.publication.revision_id
                    || existing.publication.generation != pending.publication.generation
                {
                    return Err(FeedbackError::Scope);
                }
                return Ok(key);
            }
            if state.pending_installations.len() >= MAX_ENTRIES {
                if let Some(old) = state
                    .pending_installations
                    .iter()
                    .find(|(_, item)| item.superseded)
                    .map(|(key, _)| key.clone())
                {
                    state.pending_installations.remove(&old);
                } else {
                    return Err(FeedbackError::Full);
                }
            }
            for existing in state.pending_installations.values_mut() {
                if existing.host == pending.host
                    && existing.publication.resource_id == pending.publication.resource_id
                    && existing.publication.generation < pending.publication.generation
                {
                    existing.superseded = true;
                }
            }
            let newer = state.entries.values().any(|entry| {
                entry.request.host == pending.host
                    && entry.request.resource_id == pending.publication.resource_id
                    && entry.request.generation > pending.publication.generation
            }) || state.pending_installations.values().any(|entry| {
                entry.host == pending.host
                    && entry.publication.resource_id == pending.publication.resource_id
                    && entry.publication.generation > pending.publication.generation
            });
            let mut pending = pending;
            pending.superseded = newer;
            state.pending_installations.insert(key.clone(), pending);
            Ok(key)
        })
    }

    pub fn pending_installations(&self) -> Result<Vec<(String, PendingInstallation)>> {
        self.read_locked(|state| {
            Ok(state
                .pending_installations
                .iter()
                .map(|(key, pending)| (key.clone(), pending.clone()))
                .collect())
        })
    }

    pub fn begin_installation_attempt(&self, key: &str) -> Result<()> {
        self.mutate(|state| {
            let pending = state
                .pending_installations
                .get_mut(key)
                .ok_or(FeedbackError::Scope)?;
            pending.attempts = pending.attempts.saturating_add(1);
            pending.next_attempt = Utc::now()
                + chrono::Duration::seconds(2i64.pow(pending.attempts.min(10)).min(3600));
            Ok(())
        })
    }

    pub fn complete_installation(&self, key: &str) -> Result<()> {
        self.mutate(|state| {
            let pending = state
                .pending_installations
                .get(key)
                .ok_or(FeedbackError::Scope)?;
            if !state.entries.values().any(|entry| {
                entry.request.host == pending.host
                    && entry.request.publication_id == pending.publication.publication_id
            }) {
                return Err(FeedbackError::Scope);
            }
            state.pending_installations.remove(key);
            Ok(())
        })
    }
}
