//! Pre-deploy pull of runtime files from the cloud tenant.
//!
//! Deploying replaces the running container, so runtime files (uploads,
//! AI-generated images) that are not in the local build context would be
//! lost. This step downloads the tenant's `services/` tree, backs up the
//! local copy, and applies the cloud changes before the image is built —
//! unless the caller opts out or declines the confirmation prompt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::api_client::SyncApiClient;
use crate::error::{SyncError, SyncResult};
use crate::files::FileSyncService;
use crate::{SyncConfig, SyncConfigBuilder, SyncDirection};

use super::progress::{DeployEvent, DeployProgress, DeployPrompt};
use super::request::{DeployRequest, PreSyncOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreSyncOutcome {
    Skipped,
    Completed,
    DryRun,
}

pub(super) async fn run(
    request: &DeployRequest,
    options: &PreSyncOptions,
    sync_client: Option<SyncApiClient>,
    progress: &dyn DeployProgress,
) -> SyncResult<PreSyncOutcome> {
    if options.no_sync {
        progress.event(&DeployEvent::PreSyncSkippedByFlag);
        return Ok(PreSyncOutcome::Skipped);
    }

    progress.event(&DeployEvent::PreSyncStarted);

    let dry_run = request.options.dry_run;
    if !dry_run && !options.assume_yes && !progress.confirm(&DeployPrompt::PreSync)? {
        progress.event(&DeployEvent::PreSyncDeclined);
        return Ok(PreSyncOutcome::Skipped);
    }

    let hostname = request
        .hostname
        .clone()
        .ok_or(SyncError::HostnameNotConfigured)?;
    let services_path = request.project_root.join("services");

    let sync_config = SyncConfigBuilder::new(
        request.tenant_id.clone(),
        &request.credentials.api_url,
        request.credentials.api_token.as_str(),
        services_path.to_string_lossy(),
    )
    .with_direction(SyncDirection::Pull)
    .with_dry_run(dry_run)
    .with_hostname(Some(hostname.clone()))
    .build();

    let api_client = resolve_client(request, hostname, sync_client)?;

    if dry_run {
        return run_dry_run(sync_config, api_client, progress).await;
    }

    apply_from_cloud(sync_config, api_client, options, progress).await
}

fn resolve_client(
    request: &DeployRequest,
    hostname: String,
    sync_client: Option<SyncApiClient>,
) -> SyncResult<SyncApiClient> {
    if let Some(client) = sync_client {
        return Ok(client);
    }
    Ok(SyncApiClient::new(
        &request.credentials.api_url,
        request.credentials.api_token.as_str(),
    )?
    .with_direct_sync(Some(hostname)))
}

async fn run_dry_run(
    sync_config: SyncConfig,
    api_client: SyncApiClient,
    progress: &dyn DeployProgress,
) -> SyncResult<PreSyncOutcome> {
    progress.event(&DeployEvent::SyncDryRunStarted);
    let service = FileSyncService::new(sync_config, api_client);
    match service.sync().await {
        Ok(op) if op.success => {
            progress.event(&DeployEvent::SyncDryRunFinished(&op));
            Ok(PreSyncOutcome::DryRun)
        },
        Ok(op) => {
            progress.event(&DeployEvent::SyncErrors(&op.errors));
            Err(SyncError::PreDeploySyncFailed)
        },
        Err(e) => {
            progress.event(&DeployEvent::SyncErrors(&[e.to_string()]));
            Err(SyncError::PreDeploySyncFailed)
        },
    }
}

async fn apply_from_cloud(
    sync_config: SyncConfig,
    api_client: SyncApiClient,
    options: &PreSyncOptions,
    progress: &dyn DeployProgress,
) -> SyncResult<PreSyncOutcome> {
    let services_path = std::path::PathBuf::from(&sync_config.services_path);
    let service = FileSyncService::new(sync_config, api_client);

    progress.event(&DeployEvent::SyncDownloadStarted);
    let download = service
        .download_and_diff()
        .await
        .map_err(|e| SyncError::pre_sync_stage("Download", e))?;
    progress.event(&DeployEvent::SyncDownloadFinished);

    progress.event(&DeployEvent::SyncBackupStarted);
    let backup_path = FileSyncService::backup_services(&services_path)
        .map_err(|e| SyncError::pre_sync_stage("Backup", e))?;
    progress.event(&DeployEvent::SyncBackupFinished(&backup_path));

    progress.event(&DeployEvent::SyncDiff(&download.diff));

    if !download.diff.has_changes() {
        progress.event(&DeployEvent::SyncAlreadyClean);
        return Ok(PreSyncOutcome::Completed);
    }

    let changes = download.diff.added + download.diff.modified;
    if !options.assume_yes && !progress.confirm(&DeployPrompt::ApplyChanges { count: changes })? {
        progress.event(&DeployEvent::SyncCancelled);
        return Ok(PreSyncOutcome::Skipped);
    }

    let changed_paths = download.diff.changed_paths();
    let count = FileSyncService::apply(&download.data, &services_path, Some(&changed_paths))
        .map_err(|e| SyncError::pre_sync_stage("Apply", e))?;

    progress.event(&DeployEvent::SyncApplied { count });
    Ok(PreSyncOutcome::Completed)
}
