//! Replay-protection state for manifest version monotonicity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::gateway::manifest::AutoUpdatePolicy;
use crate::gateway::manifest_version::ManifestVersion;
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub const SKEW_WINDOW_MINUTES: i64 = 5;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct LastSyncState {
    #[serde(default)]
    pub last_applied_manifest_version: Option<ManifestVersion>,
    #[serde(default)]
    pub last_applied_at: Option<String>,
    #[serde(default)]
    pub installed_plugins: Vec<String>,
    #[serde(default)]
    pub updated_plugins: Vec<String>,
    #[serde(default)]
    pub removed_plugins: Vec<String>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
    #[serde(default)]
    pub auto_update: AutoUpdatePolicy,
}

#[must_use]
pub fn last_synced_enabled_hosts() -> Option<Vec<String>> {
    read_last_sync_state().map(|state| state.enabled_hosts)
}

/// The update policy the gateway last delivered on a signed manifest.
///
/// A bridge that has never completed a sync has no delivered policy, so the
/// caller decides what an unmanaged install does rather than this returning a
/// default that looks authoritative.
#[must_use]
pub fn last_synced_auto_update_policy() -> Option<AutoUpdatePolicy> {
    read_last_sync_state().map(|state| state.auto_update)
}

fn read_last_sync_state() -> Option<LastSyncState> {
    let meta = crate::config::paths::bridge_metadata_dir()?;
    read_last_sync(&meta.join(crate::config::paths::LAST_SYNC_SENTINEL)).ok()?
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayStateError {
    #[error("read replay state {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parse replay state {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

pub fn read_last_sync(path: &Path) -> Result<Option<LastSyncState>, ReplayStateError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ReplayStateError::Read {
                path: path.display().to_string(),
                source,
            });
        },
    };
    let state = serde_json::from_slice::<LastSyncState>(&bytes).map_err(|source| {
        ReplayStateError::Parse {
            path: path.display().to_string(),
            source,
        }
    })?;
    Ok(Some(state))
}

pub fn check_replay(last: &LastSyncState, incoming: &ManifestVersion) -> Result<(), SyncError> {
    if let Some(prev) = last.last_applied_manifest_version.as_ref()
        && incoming <= prev
    {
        return Err(SyncError::ReplayedManifest {
            last: prev.to_string(),
            incoming: incoming.to_string(),
        });
    }
    Ok(())
}

pub fn check_skew(not_before: &str, now: chrono::DateTime<chrono::Utc>) -> Result<(), SyncError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(not_before).map_err(|_parse| {
        SyncError::ManifestSkew {
            not_before: not_before.to_owned(),
            now: now.to_rfc3339(),
        }
    })?;
    let nb_utc = parsed.with_timezone(&chrono::Utc);
    let window = chrono::Duration::minutes(SKEW_WINDOW_MINUTES);
    let delta = nb_utc.signed_duration_since(now);
    if delta > window || delta < -window {
        return Err(SyncError::ManifestSkew {
            not_before: not_before.to_owned(),
            now: now.to_rfc3339(),
        });
    }
    Ok(())
}
