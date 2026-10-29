//! The last bridge profile the gateway served each user, kept so every host's
//! desired settings can be rendered without a network call.
//!
//! A probe is synchronous and runs on every re-check; it compares a host
//! against the same inputs the writers render from, and this cache is where
//! those inputs come from between fetches. Entries are keyed by
//! `(gateway, user)`: the profile is always fetched as a user (it lists that
//! user's personal-key models and entitlements), so there is no anonymous
//! entry, and another user's or another gateway's profile reads as absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

use crate::gateway::types::BridgeProfile;

const CACHE_FILE: &str = "bridge-profile.json";
const SCHEMA: u32 = 2;

#[derive(Debug, Deserialize, Serialize)]
struct Entry {
    gateway: String,
    user: UserId,
    profile: BridgeProfile,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Cached {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileCacheError {
    #[error("read cached bridge profile {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
}

fn normalise(gateway: &str) -> String {
    gateway.trim_end_matches('/').to_owned()
}

fn cache_path() -> Option<std::path::PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|dir| dir.join(CACHE_FILE))
}

fn read_cached() -> Result<Cached, ProfileCacheError> {
    let Some(path) = cache_path() else {
        return Ok(Cached::default());
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Cached::default()),
        Err(source) => return Err(ProfileCacheError::Io { path, source }),
    };
    match serde_json::from_slice::<Cached>(&bytes) {
        Ok(cached) if cached.schema == SCHEMA => Ok(cached),
        _ => {
            tracing::info!(
                path = %path.display(),
                "cached bridge profile is from another bridge version; treated as absent"
            );
            Ok(Cached::default())
        },
    }
}

pub(crate) fn remember(
    gateway: &str,
    user: &UserId,
    profile: &BridgeProfile,
) -> std::io::Result<()> {
    let path = cache_path()
        .ok_or_else(|| std::io::Error::other("bridge metadata directory unavailable"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let gateway = normalise(gateway);
    // Why: an unreadable cache is replaced, not merged; it holds nothing the
    // next fetch cannot restore.
    let mut cached = read_cached().unwrap_or_default();
    cached
        .entries
        .retain(|e| !(e.gateway == gateway && &e.user == user));
    cached.schema = SCHEMA;
    cached.entries.push(Entry {
        gateway: gateway.clone(),
        user: user.clone(),
        profile: profile.clone(),
    });
    let bytes = serde_json::to_vec(&cached).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_0600(&path, &bytes)?;
    record_served_models(&gateway, profile)
}

fn record_served_models(gateway: &str, profile: &BridgeProfile) -> std::io::Result<()> {
    let active = crate::config::gateways::active().map_err(std::io::Error::other)?;
    let gateway = Some(
        systemprompt_identifiers::ValidatedUrl::try_new(gateway).map_err(std::io::Error::other)?,
    );
    crate::config::gateways::write_served_models(
        &active,
        &crate::config::gateways::ServedModels {
            gateway,
            models: profile
                .models
                .iter()
                .map(|m| m.as_str().to_owned())
                .collect(),
        },
    )
    .map_err(std::io::Error::other)
}

pub fn load_for(gateway: &str, user: &UserId) -> Result<Option<BridgeProfile>, ProfileCacheError> {
    let gateway = normalise(gateway);
    Ok(read_cached()?
        .entries
        .into_iter()
        .find(|e| e.gateway == gateway && &e.user == user)
        .map(|e| e.profile))
}

// Why: the key is the user the cached credential names; with nobody signed
// in there is no entry to read, never an anonymous one.
pub fn load() -> Result<Option<BridgeProfile>, ProfileCacheError> {
    let cfg = crate::config::load()?;
    let gateway = crate::config::gateway_url_or_default(&cfg);
    let Some(user) = super::cache::cached_user(&cfg, &gateway) else {
        return Ok(None);
    };
    load_for(gateway.as_str(), &user)
}
