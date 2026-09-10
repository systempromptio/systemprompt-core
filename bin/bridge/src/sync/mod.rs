//! Plugin/MCP sync pipeline: fetch, verify, hash-compare, and apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod apply;
mod error;
mod manifest;
mod provision;
mod replay;
mod sentinel;
mod summary;

use self::provision::{denied_inside_system_root, heal_org_plugins_scope, org_plugins_denied};
use self::sentinel::persist_last_sync;
pub use apply::{HostFailure, PLUGIN_INSTALLATION_PREFERENCE};
pub use error::{CredentialRejection, SyncError};
pub use replay::{
    LastSyncState, ReplayStateError, SKEW_WINDOW_MINUTES, check_replay, check_skew,
    last_synced_auto_update_policy, last_synced_enabled_hosts, read_last_sync,
};
pub use summary::SyncSummary;
use summary::build_summary;

use crate::config::{self, paths};
use crate::gateway::manifest::SignedManifest;
use std::fs;

pub const WATCH_FLOOR_SECS: u64 = 60;

pub fn warn_unsafe_flags(allow_unsigned: bool, force_replay: bool, allow_tofu: bool) {
    if allow_unsigned {
        tracing::warn!("--allow-unsigned bypasses signature verification");
    }
    if force_replay {
        tracing::warn!("--force-replay bypasses manifest version + skew checks");
    }
    if allow_tofu
        && matches!(
            config::pinned_pubkey_state(),
            Ok(config::PinnedPubkeyState::Unpinned)
        )
    {
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
    let operation =
        std::sync::Arc::new(std::sync::Arc::clone(&bridge.sync_lock).lock_owned().await);
    bridge
        .activity
        .ensure_persistence()
        .map_err(|source| SyncError::Persistence {
            path: crate::obs::log_dir()
                .unwrap_or_default()
                .join("activity.jsonl"),
            source,
        })?;
    bridge
        .sync_progress
        .report(&crate::progress::SyncProgress::new(
            "manifest", "manifest", 1, 1,
        ));
    let fetch = manifest::fetch_authenticated_manifest(&bridge.http).await?;
    let synced = manifest::verify_and_decode(&fetch, allow_unsigned, allow_tofu).await?;

    #[cfg_attr(
        not(target_os = "windows"),
        expect(unused_mut, reason = "only the windows heal path reassigns it")
    )]
    let mut location = paths::org_plugins_effective().ok_or(SyncError::PathUnresolvable)?;
    #[cfg(target_os = "windows")]
    if let Err(err) = check_org_plugins_scope(&synced, &location) {
        match heal_org_plugins_scope(bridge, std::sync::Arc::clone(&operation)).await? {
            Some(healed) => location = healed,
            None => return Err(err),
        }
    }
    #[cfg(not(target_os = "windows"))]
    check_org_plugins_scope(&synced, &location)?;
    if !location.path.is_dir() {
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
                provision::provision_system_org_plugins(
                    bridge,
                    &location.path,
                    std::sync::Arc::clone(&operation),
                )
                .await?;
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

    let report = match apply::apply_manifest(
        &fetch.client,
        fetch.bearer.expose(),
        bridge,
        &synced,
        &location,
    )
    .await
    {
        Ok(report) => report,
        Err(e) if denied_inside_system_root(&e, &location) => {
            let healed = heal_org_plugins_scope(bridge, std::sync::Arc::clone(&operation))
                .await?
                .ok_or_else(|| org_plugins_denied(&e, &location))?;
            tracing::warn!(
                path = %healed.path.display(),
                error = %e,
                "org-plugins re-granted after a denied plugin replacement; applying again"
            );
            apply::apply_manifest(
                &fetch.client,
                fetch.bearer.expose(),
                bridge,
                &synced,
                &healed,
            )
            .await
            .map_err(|e| org_plugins_denied(&e, &healed))?
        },
        Err(e) => return Err(SyncError::ApplyFailed(e)),
    };

    if !report.host_failures.is_empty() || !report.malformed.is_empty() {
        return Err(SyncError::Partial(Box::new(build_summary(&synced, report))));
    }
    persist_last_sync(&last_sync_path, &synced, &report, now)?;
    seed_default_model_from_profile(&fetch.client).await?;

    bridge
        .activity
        .ensure_persistence()
        .map_err(|source| SyncError::Persistence {
            path: crate::obs::log_dir()
                .unwrap_or_default()
                .join("activity.jsonl"),
            source,
        })?;
    Ok(build_summary(&synced, report))
}

#[cfg(unix)]
async fn seed_default_model_from_profile(
    client: &crate::gateway::GatewayClient,
) -> Result<(), SyncError> {
    let profile = match client.fetch_bridge_profile().await {
        Ok(profile) => profile,
        // Why: a gateway older than the profile endpoint has no default model
        // to seed; the sync itself completed and its checkpoint is written.
        Err(crate::gateway::GatewayError::HttpStatus {
            status: reqwest::StatusCode::NOT_FOUND,
            ..
        }) => return Ok(()),
        Err(e) => return Err(SyncError::Network(e.to_string())),
    };
    let rows =
        crate::install::mdm::claude_code_settings::model_picker::picker_rows(&profile.providers);
    match crate::install::mdm::claude_code_settings::apply_model_picker(&rows) {
        Ok(lines) => {
            for line in lines {
                tracing::info!(target: "bridge::install", detail = %line, "claude code model picker");
            }
        },
        Err(e) => return Err(SyncError::Network(format!("claude code model picker: {e}"))),
    }
    let Some(model) = profile.default_model.as_deref() else {
        return Ok(());
    };
    match crate::install::mdm::claude_code_settings::seed_default_model(model) {
        Ok(true) => tracing::info!(model, "seeded the default model from the bridge profile"),
        Ok(false) => tracing::debug!("settings already name a model; leaving the user's choice"),
        Err(e) => return Err(SyncError::Network(format!("seed default model: {e}"))),
    }
    Ok(())
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "matches the Linux arm's signature, which the shared call site awaits"
)]
async fn seed_default_model_from_profile(
    _client: &crate::gateway::GatewayClient,
) -> Result<(), SyncError> {
    Ok(())
}

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
