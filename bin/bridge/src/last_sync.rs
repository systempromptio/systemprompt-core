//! The `last-sync.json` sentinel: what the most recent completed sync applied.
//!
//! It sits below `update` and `sync` because both read it — the updater takes
//! its policy from the last delivered manifest, the sync pipeline uses it for
//! replay protection — and neither may reach up into the other.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::gateway::manifest::AutoUpdatePolicy;
use crate::gateway::manifest_version::ManifestVersion;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct LastSyncState {
    /// The gateway whose manifest this sync applied. Absent on a sentinel
    /// written before stamping; such a sentinel is not trusted for any gateway.
    #[serde(default)]
    pub gateway: Option<systemprompt_identifiers::ValidatedUrl>,
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

impl LastSyncState {
    /// Whether this sentinel was written for `gateway`. Replay protection and
    /// delivered policy only carry over within one gateway; a switch starts
    /// from nothing.
    #[must_use]
    pub fn belongs_to(&self, gateway: &systemprompt_identifiers::ValidatedUrl) -> bool {
        self.gateway
            .as_ref()
            .is_some_and(|g| crate::mcp_registry::same_origin(g, gateway))
    }
}

#[must_use]
pub fn last_synced_enabled_hosts() -> Option<Vec<String>> {
    read_last_sync_state().map(|state| state.enabled_hosts)
}

// Why: a bridge that has never completed a sync has no delivered policy, so the
// caller decides what an unmanaged install does rather than this returning a
// default that looks authoritative.
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
