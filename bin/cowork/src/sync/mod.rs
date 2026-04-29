mod apply;
mod error;
mod hash;
mod manifest;
mod replay;

pub use apply::ApplyError;
pub use error::SyncError;
pub use replay::{LastSyncState, SKEW_WINDOW_MINUTES, check_replay, check_skew, read_last_sync};

use crate::config::{self, paths};
use crate::gateway::manifest::SignedManifest;
use serde::Serialize;
use std::fs;

pub const WATCH_FLOOR_SECS: u64 = 60;

pub struct SyncOptions {
    pub watch: bool,
    pub interval: Option<u64>,
    pub allow_unsigned: bool,
    pub force_replay: bool,
    pub allow_tofu: bool,
}

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub identity: String,
    pub manifest_version: String,
    pub plugin_count: usize,
    pub skill_count: usize,
    pub agent_count: usize,
    pub mcp_count: usize,
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
}

impl SyncSummary {
    pub fn one_line(&self) -> String {
        let malformed_suffix = if self.malformed.is_empty() {
            String::new()
        } else {
            format!(
                " — WARNING: {} malformed plugin(s) missing claude-plugin/plugin.json: {}",
                self.malformed.len(),
                self.malformed.join(", "),
            )
        };
        format!(
            "sync ok ({}): {} plugins ({} new, {} updated, {} removed), {} skills, {} agents, {} \
             MCP — manifest {}{}",
            self.identity,
            self.plugin_count,
            self.installed.len(),
            self.updated.len(),
            self.removed.len(),
            self.skill_count,
            self.agent_count,
            self.mcp_count,
            self.manifest_version,
            malformed_suffix,
        )
    }
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
pub fn run_once(
    allow_unsigned: bool,
    force_replay: bool,
    allow_tofu: bool,
) -> Result<SyncSummary, SyncError> {
    let fetch = manifest::fetch_authenticated_manifest()?;
    manifest::verify_signature(&fetch, allow_unsigned, allow_tofu)?;

    let location = paths::org_plugins_effective().ok_or(SyncError::PathUnresolvable)?;
    if !location.path.is_dir() {
        return Err(SyncError::PathMissing {
            path: location.path.display().to_string(),
        });
    }

    let last_sync_path = paths::metadata_dir(&location.path).join(paths::LAST_SYNC_SENTINEL);
    let now = chrono::Utc::now();
    if !force_replay {
        let last_state = read_last_sync(&last_sync_path);
        check_replay(&last_state, &fetch.manifest.manifest_version)?;
        check_skew(&fetch.manifest.not_before, now)?;
    }

    let report = apply::apply_manifest(
        &fetch.client,
        fetch.bearer.expose(),
        &fetch.manifest,
        &location,
    )
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
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let sentinel = LastSyncSentinel {
        synced_at: current_iso8601(),
        manifest_version: &manifest.manifest_version,
        last_applied_manifest_version: &manifest.manifest_version,
        last_applied_at: now.to_rfc3339(),
        installed_plugins: &report.installed,
        updated_plugins: &report.updated,
        removed_plugins: &report.removed,
        mcp_server_count: manifest.managed_mcp_servers.len(),
        skill_count: manifest.skills.len(),
        agent_count: manifest.agents.len(),
        user: manifest.user.as_ref().map(|u| u.email.as_str()),
    };
    let bytes = serde_json::to_vec_pretty(&sentinel).unwrap_or_default();
    let _ = fs::write(path, bytes);
}

fn build_summary(manifest: &SignedManifest, report: apply::ApplyReport) -> SyncSummary {
    let identity = manifest
        .user
        .as_ref()
        .map(|u| u.email.clone())
        .unwrap_or_else(|| manifest.user_id.to_string());
    SyncSummary {
        identity,
        manifest_version: manifest.manifest_version.clone(),
        plugin_count: manifest.plugins.len(),
        skill_count: manifest.skills.len(),
        agent_count: manifest.agents.len(),
        mcp_count: manifest.managed_mcp_servers.len(),
        installed: report.installed,
        updated: report.updated,
        removed: report.removed,
        malformed: report.malformed,
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
    user: Option<&'a str>,
}

fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}
