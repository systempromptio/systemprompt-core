//! One sync attempt: fetch and verify the manifest, resolve the org-plugins
//! root, apply, and record the outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::sync::Arc;

use super::org_plugins_scope::check_org_plugins_scope;
use super::provision::{denied_inside_system_root, heal_org_plugins_scope, org_plugins_denied};
use super::seed_model::seed_default_model_from_profile;
use super::{
    LastSyncState, SyncError, SyncOptions, SyncSummary, acceptance, apply, apply_error_to_sync,
    build_summary, ensure_device_enrolled, manifest, prepare_run, process_lock,
    retry_pending_feedback, sentinel,
};
use crate::config::paths;
use crate::context::BridgeContext;
use crate::gateway::manifest::SignedManifest;

type OperationGuard = Arc<tokio::sync::OwnedMutexGuard<()>>;

#[tracing::instrument(level = "info", skip(bridge))]
pub(super) async fn run_attempt(
    bridge: &BridgeContext,
    options: &SyncOptions,
) -> Result<SyncSummary, SyncError> {
    retry_pending_feedback().await;
    let operation: OperationGuard = Arc::new(Arc::clone(&bridge.sync_lock).lock_owned().await);
    let _across_processes = process_lock::acquire().await?;
    prepare_run(bridge)?;
    manifest::ensure_enabled(&crate::config::load()?)?;
    let fetch = manifest::fetch_authenticated_manifest(&bridge.http, options.freshness).await?;
    let synced =
        manifest::verify_and_decode(&fetch, options.allow_unsigned, options.allow_tofu).await?;
    let run_gateway = fetch.client.base_url().clone();
    ensure_device_enrolled(bridge, &fetch, &synced.user_id).await;

    let location = resolve_org_plugins(bridge, &synced, &operation).await?;
    let now = chrono::Utc::now();
    let last_state = acceptance::accept(
        &synced,
        &run_gateway,
        options.force_replay,
        acceptance::CurrentVersion::for_run(fetch.revalidated, acceptance::CurrentVersion::Replay),
    )?;
    super::persist_envelope(&fetch)?;

    let request = apply::ApplyRequest {
        client: &fetch.client,
        bearer: &fetch.bearer,
        bridge,
        manifest: &synced,
        location: &location,
        cancel: &options.cancel,
    };
    let report = apply_with_heal(request, &operation).await?;
    let sentinel = sentinel::SentinelInputs {
        manifest: &synced,
        report: &report,
        now,
        gateway: &run_gateway,
    };
    if record_outcome(&sentinel, &last_state)? == Recorded::Partially {
        return Err(SyncError::Partial(Box::new(build_summary(&synced, report))));
    }
    seed_default_model_from_profile(&fetch.client, &synced).await?;
    prepare_persistence(bridge)?;
    drop(operation);
    Ok(build_summary(&synced, report))
}

async fn resolve_org_plugins(
    bridge: &BridgeContext,
    synced: &SignedManifest,
    operation: &OperationGuard,
) -> Result<paths::OrgPluginsLocation, SyncError> {
    let location = paths::org_plugins_effective().ok_or(SyncError::PathUnresolvable)?;
    #[cfg(target_os = "windows")]
    let location = match check_org_plugins_scope(synced, &location) {
        Ok(()) => location,
        Err(err) => heal_org_plugins_scope(bridge, Arc::clone(operation))
            .await?
            .ok_or(err)?,
    };
    #[cfg(not(target_os = "windows"))]
    check_org_plugins_scope(synced, &location)?;
    if location.path.is_dir() {
        return Ok(location);
    }
    match location.scope {
        paths::Scope::User => {
            fs::create_dir_all(&location.path).map_err(|source| SyncError::OrgPluginsCreate {
                path: location.path.clone(),
                source,
            })?;
            tracing::info!(path = %location.path.display(), "provisioned per-user org-plugins directory");
        },
        paths::Scope::System => {
            super::provision::provision_system_org_plugins(
                bridge,
                &location.path,
                Arc::clone(operation),
            )
            .await?;
        },
    }
    Ok(location)
}

async fn apply_with_heal(
    request: apply::ApplyRequest<'_>,
    operation: &OperationGuard,
) -> Result<apply::ApplyReport, SyncError> {
    let outcome = match apply::apply_manifest(&request).await {
        Ok(outcome) => outcome,
        Err(e) if denied_inside_system_root(&e, request.location) => {
            let healed = heal_org_plugins_scope(request.bridge, Arc::clone(operation))
                .await?
                .ok_or_else(|| org_plugins_denied(e, request.location))?;
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
    match outcome {
        apply::ApplyOutcome::Applied(report) => Ok(report),
        apply::ApplyOutcome::Cancelled { applied } => Err(SyncError::Cancelled { applied }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recorded {
    Fully,
    Partially,
}

fn record_outcome(
    sentinel: &sentinel::SentinelInputs<'_>,
    last_state: &LastSyncState,
) -> Result<Recorded, SyncError> {
    let meta = paths::bridge_metadata_dir().ok_or(SyncError::PathUnresolvable)?;
    let last_sync_path = meta.join(paths::LAST_SYNC_SENTINEL);
    let report = sentinel.report;
    if !report.host_failures.is_empty() || !report.malformed.is_empty() {
        sentinel.persist(
            &last_sync_path,
            sentinel::Applied::Partially { prior: last_state },
        )?;
        return Ok(Recorded::Partially);
    }
    sentinel.persist(
        &last_sync_path,
        sentinel::Applied::Fully(sentinel.manifest.manifest_version.clone()),
    )?;
    Ok(Recorded::Fully)
}

fn prepare_persistence(bridge: &BridgeContext) -> Result<(), SyncError> {
    bridge
        .activity
        .ensure_persistence()
        .map_err(|source| SyncError::Persistence {
            path: crate::obs::log_dir()
                .unwrap_or_default()
                .join("activity.jsonl"),
            source,
        })
}
