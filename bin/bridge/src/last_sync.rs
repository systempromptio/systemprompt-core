//! The `last-sync.json` sentinel: what the most recent completed sync applied.
//!
//! It sits below `update` and `sync` because both read it — the updater takes
//! its policy from the last delivered manifest, the sync pipeline uses it for
//! replay protection — and neither may reach up into the other. One struct is
//! both the writer's and every reader's shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::gateway::manifest::AutoUpdatePolicy;
use crate::gateway::manifest_version::ManifestVersion;

/// What the last completed sync applied, stamped with its gateway.
///
/// Replay protection and delivered policy only carry over within one gateway
/// (same origin); a switch starts from nothing, and a sentinel without a
/// `gateway` is trusted for none. `present_plugins` is the full set of plugin
/// directories the bridge owns after the sync — what `uninstall` may remove.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LastSyncState {
    #[serde(default)]
    pub gateway: Option<systemprompt_identifiers::ValidatedUrl>,
    #[serde(default)]
    pub synced_at: Option<String>,
    #[serde(default)]
    pub manifest_version: Option<ManifestVersion>,
    #[serde(default)]
    pub installed_plugins: Vec<String>,
    #[serde(default)]
    pub updated_plugins: Vec<String>,
    #[serde(default)]
    pub removed_plugins: Vec<String>,
    #[serde(default)]
    pub present_plugins: Vec<String>,
    #[serde(default)]
    pub mcp_server_count: usize,
    #[serde(default)]
    pub skill_count: usize,
    #[serde(default)]
    pub rule_count: usize,
    #[serde(default)]
    pub agent_count: usize,
    #[serde(default)]
    pub hook_count: usize,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
    #[serde(default)]
    pub host_model_protocols: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub auto_update: AutoUpdatePolicy,
}

impl LastSyncState {
    #[must_use]
    pub fn belongs_to(&self, gateway: &systemprompt_identifiers::ValidatedUrl) -> bool {
        self.gateway
            .as_ref()
            .is_some_and(|g| crate::mcp_registry::same_origin(g, gateway))
    }

    #[must_use]
    pub fn summary_line(&self) -> String {
        let when = self.synced_at.as_deref().unwrap_or("unknown");
        let version = self
            .manifest_version
            .as_ref()
            .map_or("?", ManifestVersion::as_str);
        format!("{when} (manifest {version})")
    }
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
    #[error("bridge metadata path unresolvable")]
    PathUnresolvable,
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
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

// Why: delivered policy (hosts, auto-update) belongs to the gateway that
// delivered it; a sentinel from another gateway is no policy at all, and a
// sentinel that cannot be read is an error the caller must not default.
pub fn delivered_state() -> Result<Option<LastSyncState>, ReplayStateError> {
    let meta =
        crate::config::paths::bridge_metadata_dir().ok_or(ReplayStateError::PathUnresolvable)?;
    let Some(state) = read_last_sync(&meta.join(crate::config::paths::LAST_SYNC_SENTINEL))? else {
        return Ok(None);
    };
    let cfg = crate::config::load()?;
    Ok(state
        .belongs_to(&crate::config::gateway_url_or_default(&cfg))
        .then_some(state))
}

pub fn last_synced_enabled_hosts() -> Result<Option<Vec<String>>, ReplayStateError> {
    Ok(delivered_state()?.map(|state| state.enabled_hosts))
}

pub fn last_synced_auto_update_policy() -> Result<Option<AutoUpdatePolicy>, ReplayStateError> {
    Ok(delivered_state()?.map(|state| state.auto_update))
}
