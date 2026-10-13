//! The last bridge profile the gateway served, kept so every host's desired
//! settings can be rendered without a network call.
//!
//! A probe is synchronous and runs on every re-check; it compares a host
//! against the same inputs the writers render from, and this cache is where
//! those inputs come from between fetches. A cache written for another
//! gateway is not this gateway's profile and reads as absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::types::BridgeProfile;

const CACHE_FILE: &str = "bridge-profile.json";

#[derive(Debug, Deserialize, Serialize)]
struct Cached {
    gateway: String,
    profile: BridgeProfile,
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileCacheError {
    #[error("read cached bridge profile {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("decode cached bridge profile {path}: {source}")]
    Json {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
}

fn normalise(gateway: &str) -> String {
    gateway.trim_end_matches('/').to_owned()
}

pub(crate) fn remember(gateway: &str, profile: &BridgeProfile) -> std::io::Result<()> {
    let dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| std::io::Error::other("bridge metadata directory unavailable"))?;
    std::fs::create_dir_all(&dir)?;
    let cached = Cached {
        gateway: normalise(gateway),
        profile: profile.clone(),
    };
    let bytes = serde_json::to_vec(&cached).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_0600(&dir.join(CACHE_FILE), &bytes)
}

pub fn load() -> Result<Option<BridgeProfile>, ProfileCacheError> {
    let Some(dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(None);
    };
    let path = dir.join(CACHE_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ProfileCacheError::Io { path, source }),
    };
    let cached: Cached = serde_json::from_slice(&bytes)
        .map_err(|source| ProfileCacheError::Json { path, source })?;
    let current = crate::config::gateway_url_or_default(&crate::config::load()?);
    if cached.gateway != normalise(current.as_str()) {
        return Ok(None);
    }
    Ok(Some(cached.profile))
}
