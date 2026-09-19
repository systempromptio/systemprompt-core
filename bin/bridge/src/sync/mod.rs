//! Plugin/MCP sync pipeline: fetch, verify, hash-compare, and apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod apply;
mod error;
mod manifest;
mod org_plugins_scope;
mod provision;
mod registry_refresh;
mod replay;
mod seed_model;
mod sentinel;
mod summary;

use self::org_plugins_scope::check_org_plugins_scope;
use self::provision::{denied_inside_system_root, heal_org_plugins_scope, org_plugins_denied};
use self::seed_model::seed_default_model_from_profile;
pub use crate::last_sync::{
    LastSyncState, ReplayStateError, last_synced_auto_update_policy, last_synced_enabled_hosts,
    read_last_sync,
};
pub use apply::{HostFailure, HostWarning, HostWarningKind, PLUGIN_INSTALLATION_PREFERENCE};
pub use error::{CredentialRejection, SyncError};
pub use provision::ProvisionError;
pub use registry_refresh::{refresh_registry, refresh_registry_for};
pub use replay::{SKEW_WINDOW_MINUTES, check_replay, check_skew};
pub use summary::SyncSummary;
use summary::build_summary;

use crate::config::{self, paths};
use crate::gateway::Freshness;
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

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub allow_unsigned: bool,
    pub force_replay: bool,
    pub allow_tofu: bool,
    pub freshness: Freshness,
    pub cancel: tokio_util::sync::CancellationToken,
}

// Why: the credential is only attribution; a sync must never fail because
// the gateway declined to enrol this install.
async fn ensure_device_enrolled(
    bridge: &crate::context::BridgeContext,
    fetch: &manifest::ManifestFetch,
    user_id: &systemprompt_identifiers::UserId,
) {
    let Ok(root) = crate::feedback::metadata_root() else {
        return;
    };
    if crate::feedback::credentials::Enrollment::load(&root, fetch.client.base_url_str()).is_ok() {
        return;
    }
    let enrolment = crate::feedback::enrol::SelfEnrolment {
        install_id: bridge.install_id().as_str(),
        user_id,
        label: crate::sysproc::host_name(),
        force_rotate: false,
    };
    if let Err(error) =
        crate::feedback::enrol::ensure_self_enrolled(&fetch.client, &fetch.bearer, &enrolment).await
    {
        tracing::warn!(%error, "device self-enrolment failed; installation feedback stays unattributed");
    }
}

async fn prepare_run(bridge: &crate::context::BridgeContext) -> Result<(), SyncError> {
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
    if let Ok(config) = config::load() {
        let gateway = config::gateway_url_or_default(&config);
        if let Err(error) = crate::feedback::retry_pending(gateway.as_str()).await {
            tracing::debug!(%error,"Installation receipts remain unacknowledged before sync");
        }
    }
    Ok(())
}

fn persist_envelope(fetch: &manifest::ManifestFetch) -> Result<(), SyncError> {
    let meta_dir = apply::metadata_dir().map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    apply::write_envelope(&meta_dir, fetch.client.base_url(), &fetch.envelope)
        .map(|_| ())
        .map_err(|e| SyncError::ApplyFailed(Box::new(e)))
}

#[tracing::instrument(level = "info", skip(bridge))]
pub async fn run_once(
    bridge: &crate::context::BridgeContext,
    options: &SyncOptions,
) -> Result<SyncSummary, SyncError> {
    let SyncOptions {
        allow_unsigned,
        force_replay,
        allow_tofu,
        freshness,
        cancel,
    } = options;
    let (allow_unsigned, force_replay, allow_tofu) = (*allow_unsigned, *force_replay, *allow_tofu);
    let operation =
        std::sync::Arc::new(std::sync::Arc::clone(&bridge.sync_lock).lock_owned().await);
    prepare_run(bridge).await?;
    let fetch = manifest::fetch_authenticated_manifest(&bridge.http, *freshness).await?;
    let synced = manifest::verify_and_decode(&fetch, allow_unsigned, allow_tofu).await?;
    let run_gateway = fetch.client.base_url().clone();
    ensure_device_enrolled(bridge, &fetch, &synced.user_id).await;
    persist_envelope(&fetch)?;

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
                fs::create_dir_all(&location.path).map_err(|source| {
                    SyncError::OrgPluginsCreate {
                        path: location.path.clone(),
                        source,
                    }
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
    let last_state = prior_checkpoint(&last_sync_path, &run_gateway)?;
    if !force_replay {
        check_skew(synced.not_before, now)?;
        if last_state.manifest_version.as_ref() == Some(&synced.manifest_version) {
            ensure_not_superseded(&run_gateway)?;
            if let Err(error) =
                crate::feedback::recover_current_manifest(fetch.client.base_url_str(), &synced)
                    .await
            {
                tracing::debug!(%error, "Pending installation recovery remains unacknowledged");
            }
        }
        check_replay(&last_state, &synced.manifest_version)?;
    }
    ensure_not_superseded(&run_gateway)?;

    let request = apply::ApplyRequest {
        client: &fetch.client,
        bearer: &fetch.bearer,
        bridge,
        manifest: &synced,
        location: &location,
        cancel,
    };
    let outcome = match apply::apply_manifest(&request).await {
        Ok(outcome) => outcome,
        Err(e) if denied_inside_system_root(&e, &location) => {
            let healed = heal_org_plugins_scope(bridge, std::sync::Arc::clone(&operation))
                .await?
                .ok_or_else(|| org_plugins_denied(e, &location))?;
            tracing::warn!(
                path = %healed.path.display(),
                "org-plugins re-granted after a denied plugin replacement; applying again"
            );
            let healed_request = apply::ApplyRequest {
                location: &healed,
                ..request
            };
            apply::apply_manifest(&healed_request)
                .await
                .map_err(|e| org_plugins_denied(e, &healed))?
        },
        Err(e) => return Err(apply_error_to_sync(e)),
    };
    let report = match outcome {
        apply::ApplyOutcome::Applied(report) => report,
        apply::ApplyOutcome::Cancelled { applied } => {
            return Err(SyncError::Cancelled { applied });
        },
    };

    let sentinel = sentinel::SentinelInputs {
        manifest: &synced,
        report: &report,
        now,
        gateway: &run_gateway,
    };
    if !report.host_failures.is_empty() || !report.malformed.is_empty() {
        sentinel.persist(
            &last_sync_path,
            sentinel::Applied::Partially { prior: &last_state },
        )?;
        return Err(SyncError::Partial(Box::new(build_summary(&synced, report))));
    }
    sentinel.persist(
        &last_sync_path,
        sentinel::Applied::Fully(synced.manifest_version.clone()),
    )?;
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

fn prior_checkpoint(
    last_sync_path: &std::path::Path,
    run_gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<LastSyncState, SyncError> {
    match read_last_sync(last_sync_path) {
        // Why: manifest versions are per gateway; the previous gateway's
        // version says nothing about whether this one is a replay.
        Ok(Some(s)) if s.belongs_to(run_gateway) => Ok(s),
        Ok(_) => Ok(LastSyncState::default()),
        Err(e) => {
            tracing::error!(error = %e, "replay state file is corrupt; refusing to apply");
            Err(SyncError::from(e))
        },
    }
}

fn ensure_not_superseded(
    run_gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<(), SyncError> {
    apply::check_not_superseded(run_gateway).map_err(apply_error_to_sync)
}

fn apply_error_to_sync(e: apply::ApplyError) -> SyncError {
    match e {
        apply::ApplyError::Superseded {
            started_for,
            current,
        } => SyncError::Superseded {
            started_for,
            current,
        },
        other => SyncError::ApplyFailed(Box::new(other)),
    }
}
