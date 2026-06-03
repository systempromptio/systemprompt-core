pub(crate) mod apply;
mod error;
mod hash;
pub mod host_sync;
mod manifest;
mod replay;

pub use apply::{
    ApplyError, HostFailure, PLUGIN_INSTALLATION_PREFERENCE, TomlError, prune_stale_locations_in,
    render_plugin_json, write_synthetic_plugin,
};
pub use error::SyncError;
pub(crate) use hash::safe_id_segment;
pub use host_sync::{HostSync, HostSyncCtx};
pub use replay::{
    LastSyncState, ReplayStateError, SKEW_WINDOW_MINUTES, check_replay, check_skew, read_last_sync,
};

use crate::config::{self, paths};
use crate::gateway::manifest::SignedManifest;
use serde::Serialize;
use std::fs;

pub const WATCH_FLOOR_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub identity: String,
    pub manifest_version: String,
    pub plugin_count: usize,
    pub skill_count: usize,
    pub agent_count: usize,
    pub hook_count: usize,
    pub mcp_count: usize,
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
    pub host_failures: Vec<HostFailure>,
}

impl SyncSummary {
    #[must_use]
    pub fn one_line(&self) -> String {
        let status = if self.host_failures.is_empty() {
            "sync ok"
        } else {
            "sync PARTIAL"
        };
        let malformed_suffix = if self.malformed.is_empty() {
            String::new()
        } else {
            format!(
                " — WARNING: {} malformed plugin(s) missing claude-plugin/plugin.json: {}",
                self.malformed.len(),
                self.malformed.join(", "),
            )
        };
        let host_suffix = if self.host_failures.is_empty() {
            String::new()
        } else {
            let detail = self
                .host_failures
                .iter()
                .map(|f| format!("{} ({})", f.host_id, first_line(&f.error)))
                .collect::<Vec<_>>()
                .join("; ");
            format!(
                " — {} host(s) failed: {} — see bridge.log",
                self.host_failures.len(),
                detail,
            )
        };
        format!(
            "{status} ({}): {} plugins ({} new, {} updated, {} removed), {} skills, {} agents, {} \
             hooks, {} MCP — manifest {}{}{}",
            self.identity,
            self.plugin_count,
            self.installed.len(),
            self.updated.len(),
            self.removed.len(),
            self.skill_count,
            self.agent_count,
            self.hook_count,
            self.mcp_count,
            self.manifest_version,
            malformed_suffix,
            host_suffix,
        )
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_owned()
}

pub fn warn_unsafe_flags(allow_unsigned: bool, force_replay: bool, allow_tofu: bool) {
    if allow_unsigned {
        tracing::warn!("--allow-unsigned bypasses signature verification");
    }
    if force_replay {
        tracing::warn!("--force-replay bypasses manifest version + skew checks");
    }
    if allow_tofu && config::pinned_pubkey().is_none() {
        tracing::warn!(
            "--allow-tofu enables trust-on-first-use pubkey fetch over the gateway channel; this \
             is insecure if the gateway is not authenticated yet"
        );
    }
}

#[tracing::instrument(level = "info")]
pub async fn run_once(
    allow_unsigned: bool,
    force_replay: bool,
    allow_tofu: bool,
) -> Result<SyncSummary, SyncError> {
    let fetch = manifest::fetch_authenticated_manifest().await?;
    manifest::verify_signature(&fetch, allow_unsigned, allow_tofu).await?;

    let location = paths::org_plugins_effective().ok_or(SyncError::PathUnresolvable)?;
    if !location.path.is_dir() {
        return Err(SyncError::PathMissing {
            path: location.path.display().to_string(),
        });
    }

    let meta = paths::bridge_metadata_dir().ok_or(SyncError::PathUnresolvable)?;
    let last_sync_path = meta.join(paths::LAST_SYNC_SENTINEL);
    let now = chrono::Utc::now();
    if !force_replay {
        let last_state = match read_last_sync(&last_sync_path) {
            Ok(Some(s)) => s,
            Ok(None) => LastSyncState::default(),
            Err(e) => {
                tracing::error!(error = %e, "replay state file is corrupt; refusing to apply");
                return Err(SyncError::from(e));
            },
        };
        check_replay(&last_state, &fetch.manifest.manifest_version)?;
        check_skew(&fetch.manifest.not_before, now)?;
    }

    let report = apply::apply_manifest(
        &fetch.client,
        fetch.bearer.expose(),
        &fetch.manifest,
        &location,
    )
    .await
    .map_err(SyncError::ApplyFailed)?;

    persist_last_sync(&last_sync_path, &fetch.manifest, &report, now);

    Ok(build_summary(&fetch.manifest, report))
}

fn persist_last_sync(
    path: &std::path::Path,
    manifest: &SignedManifest,
    report: &apply::ApplyReport,
    now: chrono::DateTime<chrono::Utc>,
) {
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::warn!(error = %e, dir = %parent.display(), "sync: sentinel parent mkdir failed");
        return;
    }
    let sentinel = LastSyncSentinel {
        synced_at: current_iso8601(),
        manifest_version: manifest.manifest_version.as_str(),
        last_applied_manifest_version: manifest.manifest_version.as_str(),
        last_applied_at: now.to_rfc3339(),
        installed_plugins: &report.installed,
        updated_plugins: &report.updated,
        removed_plugins: &report.removed,
        mcp_server_count: manifest.managed_mcp_servers.len(),
        skill_count: manifest.skills.len(),
        agent_count: manifest.agents.len(),
        hook_count: manifest.hooks.len(),
        user: manifest.user.as_ref().map(|u| u.email.as_str()),
        enabled_hosts: &manifest.enabled_hosts,
        host_model_protocols: &manifest.host_model_protocols,
    };
    let bytes = match serde_json::to_vec_pretty(&sentinel) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "sync: sentinel serialize failed");
            return;
        },
    };
    if let Err(e) = fs::write(path, bytes) {
        tracing::warn!(error = %e, path = %path.display(), "sync: sentinel write failed");
    }
}

fn build_summary(manifest: &SignedManifest, report: apply::ApplyReport) -> SyncSummary {
    let identity = manifest
        .user
        .as_ref()
        .map_or_else(|| manifest.user_id.to_string(), |u| u.email.clone());
    SyncSummary {
        identity,
        manifest_version: manifest.manifest_version.to_string(),
        plugin_count: manifest.plugins.len(),
        skill_count: manifest.skills.len(),
        agent_count: manifest.agents.len(),
        hook_count: manifest.hooks.len(),
        mcp_count: manifest.managed_mcp_servers.len(),
        installed: report.installed,
        updated: report.updated,
        removed: report.removed,
        malformed: report.malformed,
        host_failures: report.host_failures,
    }
}

#[derive(Serialize)]
struct LastSyncSentinel<'a> {
    synced_at: String,
    manifest_version: &'a str,
    last_applied_manifest_version: &'a str,
    last_applied_at: String,
    installed_plugins: &'a [String],
    updated_plugins: &'a [String],
    removed_plugins: &'a [String],
    mcp_server_count: usize,
    skill_count: usize,
    agent_count: usize,
    hook_count: usize,
    user: Option<&'a str>,
    enabled_hosts: &'a [String],
    host_model_protocols: &'a std::collections::BTreeMap<String, Vec<String>>,
}

fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}
