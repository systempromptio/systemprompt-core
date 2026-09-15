//! The last-sync sentinel: the replay checkpoint and the summary the GUI and
//! `validate` read back, written atomically after a fully applied manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::apply::ApplyReport;
use super::error::SyncError;
use crate::gateway::manifest::SignedManifest;
use crate::last_sync::LastSyncState;

pub(super) fn persist_last_sync(
    path: &std::path::Path,
    manifest: &SignedManifest,
    report: &ApplyReport,
    now: chrono::DateTime<chrono::Utc>,
    gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<(), SyncError> {
    let state = LastSyncState {
        gateway: Some(gateway.clone()),
        synced_at: Some(now.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)),
        manifest_version: Some(manifest.manifest_version.clone()),
        installed_plugins: report.installed.clone(),
        updated_plugins: report.updated.clone(),
        removed_plugins: report.removed.clone(),
        present_plugins: manifest
            .plugins
            .iter()
            .map(|p| p.id.as_str().to_owned())
            .collect(),
        mcp_server_count: manifest.managed_mcp_servers.len(),
        skill_count: manifest.skills.len(),
        rule_count: manifest.rules.len(),
        agent_count: manifest.agents.len(),
        hook_count: manifest.hooks.len(),
        user: manifest.user.as_ref().map(|u| u.email.as_str().to_owned()),
        enabled_hosts: manifest.enabled_hosts.clone(),
        host_model_protocols: manifest.host_model_protocols.clone(),
        auto_update: manifest.auto_update,
    };
    let bytes = serde_json::to_vec_pretty(&state).map_err(|e| SyncError::Persistence {
        path: path.to_owned(),
        source: std::io::Error::other(e),
    })?;
    crate::fsutil::atomic_write_0600(path, &bytes).map_err(|source| SyncError::Persistence {
        path: path.to_owned(),
        source,
    })
}
