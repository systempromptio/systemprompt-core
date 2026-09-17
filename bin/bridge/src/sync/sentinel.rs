//! The last-sync sentinel: the replay checkpoint and the summary the GUI and
//! `validate` read back, written atomically after every applied manifest.
//!
//! A partial apply (a host emitter failed, a plugin was malformed) still
//! records what landed — the plugins, the delivered policy, the failures —
//! but keeps the previously applied manifest version as the replay
//! checkpoint so the next attempt at the same manifest is not refused as a
//! replay.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::apply::{ApplyReport, HostFailure};
use super::error::SyncError;
use crate::gateway::manifest::SignedManifest;
use crate::gateway::manifest_version::ManifestVersion;
use crate::last_sync::LastSyncState;

pub(super) struct SentinelInputs<'a> {
    pub manifest: &'a SignedManifest,
    pub report: &'a ApplyReport,
    pub now: chrono::DateTime<chrono::Utc>,
    pub gateway: &'a systemprompt_identifiers::ValidatedUrl,
}

impl SentinelInputs<'_> {
    pub(super) fn persist(
        &self,
        path: &std::path::Path,
        applied_version: Option<ManifestVersion>,
    ) -> Result<(), SyncError> {
        let state = last_sync_state(
            self.manifest,
            self.report,
            self.now,
            self.gateway,
            applied_version,
        );
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| SyncError::Persistence {
            path: path.to_owned(),
            source: std::io::Error::other(e),
        })?;
        crate::fsutil::atomic_write_0600(path, &bytes).map_err(|source| SyncError::Persistence {
            path: path.to_owned(),
            source,
        })
    }
}

#[must_use]
pub(crate) fn last_sync_state(
    manifest: &SignedManifest,
    report: &ApplyReport,
    now: chrono::DateTime<chrono::Utc>,
    gateway: &systemprompt_identifiers::ValidatedUrl,
    applied_version: Option<ManifestVersion>,
) -> LastSyncState {
    LastSyncState {
        gateway: Some(gateway.clone()),
        synced_at: Some(now.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)),
        manifest_version: applied_version,
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
        host_failures: report
            .host_failures
            .iter()
            .map(HostFailure::sentinel_line)
            .collect(),
        malformed_plugins: report.malformed.clone(),
    }
}
