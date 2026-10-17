//! Reloadable process-wide secrets store.
//!
//! Callers hold `&'static Secrets`, so a reload cannot free the generation it
//! replaces: each accepted generation is `Box::leak`ed and the pointer in
//! [`SECRETS`] is swapped. A leaked generation is never zeroized — the same
//! exposure the former `OnceLock` had for the single generation it held.
//! Growth is bounded because a reload only leaks when a value actually
//! changed, and scheduled reloads run no more often than the 60 s floor on
//! `secrets.refresh_secs` (a `file` source) or `secrets.external.refresh_secs`.
//!
//! The keys in [`PINNED_SECRET_KEYS`] are fixed for the life of the process:
//! data sealed, signed or connected with the old value would stop verifying,
//! so a reload that changes one is refused and the current generation stays.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_manifest::profile::{MIN_EXTERNAL_REFRESH_SECS, SecretsConfig, SecretsSource};
use systemprompt_manifest::secrets::Secrets;

use super::{SecretsBootstrap, SecretsBootstrapError, loader};
use crate::bootstrap::profile::ProfileBootstrap;
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

static SECRETS: RwLock<Option<&'static Secrets>> = RwLock::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static STATUS: RwLock<Option<SecretsStatus>> = RwLock::new(None);
static RELOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

pub(super) fn current() -> Option<&'static Secrets> {
    *SECRETS.read().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn install(secrets: Secrets) -> Result<&'static Secrets, SecretsBootstrapError> {
    let mut slot = SECRETS.write().unwrap_or_else(PoisonError::into_inner);
    if slot.is_some() {
        return Err(SecretsBootstrapError::AlreadyInitialized);
    }
    let leaked: &'static Secrets = Box::leak(Box::new(secrets));
    *slot = Some(leaked);
    drop(slot);
    GENERATION.store(1, Ordering::SeqCst);
    *STATUS.write().unwrap_or_else(PoisonError::into_inner) = Some(SecretsStatus {
        source: active_source_name(),
        generation: 1,
        initialised_at: Utc::now(),
        last_reload_at: None,
        last_error: None,
    });
    Ok(leaked)
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

impl SecretsBootstrap {
    pub async fn reload() -> ConfigResult<ReloadOutcome> {
        let _guard = RELOAD_LOCK.lock().await;
        let active = current().ok_or(SecretsBootstrapError::NotInitialized)?;
        let attempt = Self::load_candidate(active).await;
        record_reload(attempt.as_ref().err());
        let (fresh, changed_keys) = attempt?;

        if changed_keys.is_empty() {
            return Ok(ReloadOutcome {
                generation: GENERATION.load(Ordering::SeqCst),
                changed_keys,
                swapped: false,
            });
        }

        let leaked: &'static Secrets = Box::leak(Box::new(fresh));
        *SECRETS.write().unwrap_or_else(PoisonError::into_inner) = Some(leaked);
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(status) = STATUS
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            status.generation = generation;
        }
        tracing::info!(generation, changed = ?changed_keys, "secrets reloaded");
        Ok(ReloadOutcome {
            generation,
            changed_keys,
            swapped: true,
        })
    }

    async fn load_candidate(active: &Secrets) -> ConfigResult<(Secrets, Vec<String>)> {
        let fresh = loader::load_from_profile_config().await?;
        Self::validate_identity(&fresh)?;
        let changed = diff_secrets(active, &fresh).map_err(ConfigError::from)?;
        Ok((fresh, changed))
    }

    #[must_use]
    pub fn generation() -> u64 {
        GENERATION.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn status() -> Option<SecretsStatus> {
        STATUS
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn refresh_interval() -> Option<Duration> {
        refresh_interval_for(ProfileBootstrap::get().ok()?.secrets.as_ref()?)
    }

    pub fn key_names() -> Result<Vec<String>, SecretsBootstrapError> {
        let active = current().ok_or(SecretsBootstrapError::NotInitialized)?;
        Ok(as_fields(active)?
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

fn record_reload(error: Option<&ConfigError>) {
    if let Some(status) = STATUS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        status.last_reload_at = Some(Utc::now());
        status.last_error = error.map(ToString::to_string);
    }
}

fn active_source_name() -> String {
    ProfileBootstrap::get()
        .ok()
        .and_then(|profile| profile.secrets.as_ref())
        .map_or("env", |config| config.source_name())
        .to_owned()
}
