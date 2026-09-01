//! Plugin/MCP sync pipeline: fetch, verify, hash-compare, and apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod apply;
mod error;
mod manifest;
mod replay;
mod summary;

pub use apply::{HostFailure, PLUGIN_INSTALLATION_PREFERENCE};
pub use error::{CredentialRejection, SyncError};
pub use replay::{
    LastSyncState, ReplayStateError, SKEW_WINDOW_MINUTES, check_replay, check_skew, read_last_sync,
};
pub use summary::SyncSummary;
use summary::build_summary;

use crate::config::{self, paths};
use crate::gateway::manifest::SignedManifest;
use serde::Serialize;
use std::fs;

pub const WATCH_FLOOR_SECS: u64 = 60;

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
    bridge: &crate::context::BridgeContext,
    allow_unsigned: bool,
    force_replay: bool,
    allow_tofu: bool,
) -> Result<SyncSummary, SyncError> {
    let fetch = manifest::fetch_authenticated_manifest().await?;
    let synced = manifest::verify_and_decode(&fetch, allow_unsigned, allow_tofu).await?;

    #[cfg_attr(
        not(target_os = "windows"),
        expect(unused_mut, reason = "only the windows heal path reassigns it")
    )]
    let mut location = paths::org_plugins_effective().ok_or(SyncError::PathUnresolvable)?;
    #[cfg(target_os = "windows")]
    if let Err(err) = check_org_plugins_scope(&synced, &location) {
        match heal_org_plugins_scope().await {
            Some(healed) => location = healed,
            None => return Err(err),
        }
    }
    #[cfg(not(target_os = "windows"))]
    check_org_plugins_scope(&synced, &location)?;
    if !location.path.is_dir() {
        // Why: only the macOS system path needs `sudo install --apply` — the
        // per-user location (Windows/Linux) is writable by this process, so a
        // missing directory on a fresh install is provisioned here instead of
        // bouncing the user to a sudo command that does not apply to their OS.
        match location.scope {
            paths::Scope::User => {
                fs::create_dir_all(&location.path).map_err(|e| {
                    SyncError::Network(format!(
                        "could not create org-plugins directory at {}: {e}",
                        location.path.display()
                    ))
                })?;
                tracing::info!(path = %location.path.display(), "provisioned per-user org-plugins directory");
            },
            paths::Scope::System => {
                return Err(SyncError::PathMissing {
                    bin: crate::brand::brand().binary_name,
                    path: location.path.display().to_string(),
                });
            },
        }
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
        check_replay(&last_state, &synced.manifest_version)?;
        check_skew(&synced.not_before, now)?;
    }

    let report = apply::apply_manifest(
        &fetch.client,
        fetch.bearer.expose(),
        bridge,
        &synced,
        &location,
    )
    .await
    .map_err(SyncError::ApplyFailed)?;

    seed_default_model_from_profile(&fetch.client).await;

    persist_last_sync(&last_sync_path, &synced, &report, now);

    Ok(build_summary(&synced, report))
}

// Why: the fleet's default model is server policy (`GET /v1/bridge/profile`),
// applied on sync rather than in the synchronous `install --apply` so a policy
// change reaches existing installs on their next scheduled sync. Best-effort
// throughout: an absent field or an unreachable gateway leaves the model
// choice as it was, and never fails a sync whose manifest applied cleanly.
#[cfg(target_os = "linux")]
async fn seed_default_model_from_profile(client: &crate::gateway::GatewayClient) {
    let Ok(profile) = client.fetch_bridge_profile().await else {
        return;
    };
    let Some(model) = profile.default_model.as_deref() else {
        return;
    };
    match crate::install::mdm::linux::seed_default_model(model) {
        Ok(true) => tracing::info!(model, "seeded the default model from the bridge profile"),
        Ok(false) => tracing::debug!("settings already name a model; leaving the user's choice"),
        Err(e) => tracing::warn!(error = %e, "could not seed the default model"),
    }
}

#[cfg(not(target_os = "linux"))]
#[expect(
    clippy::unused_async,
    reason = "matches the Linux arm's signature, which the shared call site awaits"
)]
async fn seed_default_model_from_profile(_client: &crate::gateway::GatewayClient) {}

// Why: on Windows, Cowork scans only the system org-plugins path. Writing the
// user-scope fallback there succeeds but is invisible to Cowork, so a sync
// that targets the Claude Desktop host from a non-elevated process must fail
// loudly instead of reporting success. Other platforms either have no fallback
// (macOS) or no Cowork desktop app, and the gateway enables all known hosts
// by default, so the check cannot be platform-neutral.
#[cfg(target_os = "windows")]
fn check_org_plugins_scope(
    manifest: &SignedManifest,
    location: &paths::OrgPluginsLocation,
) -> Result<(), SyncError> {
    if manifest.enabled_hosts.iter().any(|h| h == "claude-desktop")
        && let paths::FallbackReason::SystemUnwritable { system_path } = &location.reason
    {
        return Err(SyncError::OrgPluginsNeedElevation {
            bin: crate::brand::brand().binary_name,
            system_path: system_path.display().to_string(),
        });
    }
    Ok(())
}

// Why: the double-click GUI flow has no CLI step, so the sync itself must be
// able to raise the single administrator prompt that provisions org-plugins.
// One attempt per process: a declined prompt must not re-fire from the GUI
// auto-sync, tray retries, or a `sync --watch` loop.
#[cfg(target_os = "windows")]
async fn heal_org_plugins_scope() -> Option<paths::OrgPluginsLocation> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    if ATTEMPTED.swap(true, Ordering::SeqCst) {
        return None;
    }
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    if let Err(e) = fs::create_dir_all(&stage_dir) {
        tracing::warn!(error = %e, "could not create staging dir for org-plugins provisioning");
        return None;
    }
    tracing::info!(
        path = %org.path.display(),
        "requesting one-time administrator approval to provision org-plugins for Cowork"
    );
    let job = crate::install::elevated_job::ElevatedJob {
        clear_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        reg_path: None,
        org_plugins: Some(org),
    };
    let outcome = tokio::task::spawn_blocking(move || {
        crate::install::elevated_job::elevate_and_run(&stage_dir, &job)
    })
    .await;
    match outcome {
        Ok(Ok(())) => paths::org_plugins_effective().filter(|l| l.scope == paths::Scope::System),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "org-plugins provisioning was not completed");
            None
        },
        Err(e) => {
            tracing::warn!(error = %e, "org-plugins provisioning task failed");
            None
        },
    }
}

#[cfg(not(target_os = "windows"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "signature must match the windows variant so run_once stays cfg-free"
)]
const fn check_org_plugins_scope(
    _manifest: &SignedManifest,
    _location: &paths::OrgPluginsLocation,
) -> Result<(), SyncError> {
    Ok(())
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
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}
