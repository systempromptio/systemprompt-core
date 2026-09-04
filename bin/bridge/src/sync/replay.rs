//! Replay-protection state for manifest version monotonicity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
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
    // Why: written by `LastSyncSentinel` and read back here so callers outside
    // the GUI can tell whether the instance enables a host before offering to
    // enrol a client into it. Read-only for this struct: the sentinel is the
    // writer, so the field cannot be lost by a round trip through here.
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
}

/// The hosts the last synced manifest enabled, or `None` when no manifest has
/// been applied on this machine yet.
///
/// `None` is not "nothing is enabled": a bridge installed a minute ago has no
/// record at all, and a caller must not read that as a refusal.
#[must_use]
pub fn last_synced_enabled_hosts() -> Option<Vec<String>> {
    let meta = crate::config::paths::bridge_metadata_dir()?;
    let state = read_last_sync(&meta.join(crate::config::paths::LAST_SYNC_SENTINEL)).ok()??;
    Some(state.enabled_hosts)
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
