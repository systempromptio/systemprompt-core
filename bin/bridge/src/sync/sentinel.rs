//! The last-sync sentinel: the replay checkpoint and the summary the GUI and
//! `validate` read back, written atomically after a fully applied manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use super::apply::ApplyReport;
use super::error::SyncError;
use crate::gateway::manifest::SignedManifest;

pub(super) fn persist_last_sync(
    path: &std::path::Path,
    manifest: &SignedManifest,
    report: &ApplyReport,
    now: chrono::DateTime<chrono::Utc>,
    gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<(), SyncError> {
    let sentinel = LastSyncSentinel {
        gateway: gateway.as_str(),
        synced_at: current_iso8601(),
        manifest_version: manifest.manifest_version.as_str(),
        last_applied_manifest_version: manifest.manifest_version.as_str(),
        last_applied_at: now.to_rfc3339(),
        installed_plugins: &report.installed,
        updated_plugins: &report.updated,
        removed_plugins: &report.removed,
        mcp_server_count: manifest.managed_mcp_servers.len(),
        skill_count: manifest.skills.len(),
        rule_count: manifest.rules.len(),
        agent_count: manifest.agents.len(),
        hook_count: manifest.hooks.len(),
        user: manifest.user.as_ref().map(|u| u.email.as_str()),
        enabled_hosts: &manifest.enabled_hosts,
        host_model_protocols: &manifest.host_model_protocols,
        auto_update: manifest.auto_update,
    };
    let bytes = serde_json::to_vec_pretty(&sentinel).map_err(|e| SyncError::Persistence {
        path: path.to_owned(),
        source: std::io::Error::other(e),
    })?;
    crate::fsutil::atomic_write_0600(path, &bytes).map_err(|source| SyncError::Persistence {
        path: path.to_owned(),
        source,
    })
}

#[derive(Serialize)]
struct LastSyncSentinel<'a> {
    gateway: &'a str,
    synced_at: String,
    manifest_version: &'a str,
    last_applied_manifest_version: &'a str,
    last_applied_at: String,
    installed_plugins: &'a [String],
    updated_plugins: &'a [String],
    removed_plugins: &'a [String],
    mcp_server_count: usize,
    skill_count: usize,
    rule_count: usize,
    agent_count: usize,
    hook_count: usize,
    user: Option<&'a str>,
    enabled_hosts: &'a [String],
    host_model_protocols: &'a std::collections::BTreeMap<String, Vec<String>>,
    auto_update: crate::gateway::manifest::AutoUpdatePolicy,
}

fn current_iso8601() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}
