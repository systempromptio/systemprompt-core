//! Reloadable secrets snapshot owned by [`SecretsService`].
//!
//! Callers hold an `Arc<Secrets>` snapshot, so a reload cannot free the
//! generation it replaces: the service swaps the pointer and the old
//! generation lives until its last reader drops it. Scheduled reloads run no
//! more often than the 60 s floor on `secrets.refresh_secs` (a `file` source)
//! or `secrets.external.refresh_secs`.
//!
//! The keys in [`PINNED_SECRET_KEYS`] are fixed for the life of the process:
//! data sealed, signed or connected with the old value would stop verifying,
//! so a reload that changes one is refused and the current generation stays.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_manifest::profile::{MIN_EXTERNAL_REFRESH_SECS, SecretsConfig, SecretsSource};
use systemprompt_manifest::secrets::Secrets;

use super::{SecretsBootstrapError, SecretsService, loader};
use crate::error::{ConfigError, ConfigResult};

pub const PINNED_SECRET_KEYS: &[&str] = &[
    "encryption_master_key",
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "database_url",
    "database_write_url",
    "database_session_url",
    "signing_key_pem",
    "jwt_secret",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReloadOutcome {
    pub generation: u64,
    pub changed_keys: Vec<String>,
    pub swapped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecretsStatus {
    pub source: String,
    pub generation: u64,
    pub initialised_at: DateTime<Utc>,
    pub last_reload_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub(super) struct SecretsState {
    secrets: RwLock<Arc<Secrets>>,
    generation: AtomicU64,
    status: RwLock<SecretsStatus>,
    reload_lock: tokio::sync::Mutex<()>,
}

impl SecretsState {
    pub(super) fn new(secrets: Secrets, source: String) -> Self {
        Self {
            secrets: RwLock::new(Arc::new(secrets)),
            generation: AtomicU64::new(1),
            status: RwLock::new(SecretsStatus {
                source,
                generation: 1,
                initialised_at: Utc::now(),
                last_reload_at: None,
                last_error: None,
            }),
            reload_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub(super) fn current(&self) -> Arc<Secrets> {
        Arc::clone(&self.secrets.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn record_reload(&self, error: Option<&ConfigError>) {
        let mut status = self.status.write().unwrap_or_else(PoisonError::into_inner);
        status.last_reload_at = Some(Utc::now());
        status.last_error = error.map(ToString::to_string);
    }
}

pub fn diff_secrets(old: &Secrets, new: &Secrets) -> Result<Vec<String>, SecretsBootstrapError> {
    let old = as_fields(old)?;
    let new = as_fields(new)?;
    let mut changed: Vec<String> = old
        .keys()
        .chain(new.keys())
        .filter(|key| old.get(*key) != new.get(*key))
        .cloned()
        .collect();
    changed.sort_unstable();
    changed.dedup();
    if let Some(key) = changed
        .iter()
        .find(|key| PINNED_SECRET_KEYS.contains(&key.as_str()))
    {
        return Err(SecretsBootstrapError::ReloadRejected { key: key.clone() });
    }
    Ok(changed)
}

// JSON: the serialized `secrets.json` shape, compared field by field.
fn as_fields(
    secrets: &Secrets,
) -> Result<serde_json::Map<String, serde_json::Value>, SecretsBootstrapError> {
    match serde_json::to_value(secrets).map_err(SecretsBootstrapError::DocumentEncode)? {
        serde_json::Value::Object(fields) => Ok(fields),
        _ => Ok(serde_json::Map::new()),
    }
}

impl SecretsService {
    pub async fn reload(&self) -> ConfigResult<ReloadOutcome> {
        let state = &self.state;
        let _guard = state.reload_lock.lock().await;
        let active = state.current();
        let attempt = self.load_candidate(&active).await;
        state.record_reload(attempt.as_ref().err());
        let (fresh, changed_keys) = attempt?;

        if changed_keys.is_empty() {
            return Ok(ReloadOutcome {
                generation: state.generation.load(Ordering::SeqCst),
                changed_keys,
                swapped: false,
            });
        }

        *state.secrets.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(fresh);
        let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
        state
            .status
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .generation = generation;
        tracing::info!(generation, changed = ?changed_keys, "secrets reloaded");
        Ok(ReloadOutcome {
            generation,
            changed_keys,
            swapped: true,
        })
    }

    async fn load_candidate(&self, active: &Secrets) -> ConfigResult<(Secrets, Vec<String>)> {
        let fresh = loader::load_from_profile_config(&self.profile).await?;
        Self::validate_identity(&self.profile, &fresh)?;
        let changed = diff_secrets(active, &fresh).map_err(ConfigError::from)?;
        Ok((fresh, changed))
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.state.generation.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn status(&self) -> SecretsStatus {
        self.state
            .status
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn refresh_interval(&self) -> Option<Duration> {
        refresh_interval_for(self.profile.secrets.as_ref()?)
    }

    pub fn key_names(&self) -> Result<Vec<String>, SecretsBootstrapError> {
        let active = self.state.current();
        Ok(as_fields(&active)?
            .into_iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(key, _)| key)
            .collect())
    }
}

#[must_use]
pub fn refresh_interval_for(config: &SecretsConfig) -> Option<Duration> {
    let secs = match config.source {
        SecretsSource::File => config.refresh_secs?,
        SecretsSource::External => config.external.as_ref()?.refresh_secs?,
        SecretsSource::Env | SecretsSource::Vault => return None,
    };
    Some(Duration::from_secs(secs.max(MIN_EXTERNAL_REFRESH_SECS)))
}
