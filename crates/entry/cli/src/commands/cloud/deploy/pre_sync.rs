use std::path::Path;

use anyhow::Result;
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_sync::{FileSyncService, SyncApiClient, SyncOperationResult};

use crate::cli_settings::CliConfig;
use crate::interactive::confirm_optional;

use super::pre_sync_config::build_sync_config;
use super::pre_sync_display::{display_destructive_warning, display_diff, handle_sync_result};

pub struct PreSyncConfig {
    pub no_sync: bool,
    pub yes: bool,
    pub dry_run: bool,
}

pub struct PreSyncResult {
    pub dry_run: bool,
}

impl PreSyncResult {
    pub(super) const fn skipped() -> Self {
        Self { dry_run: false }
    }

    pub(super) const fn dry_run() -> Self {
        Self { dry_run: true }
    }

    pub(super) const fn success() -> Self {
        Self { dry_run: false }
    }
}

pub async fn execute(
    tenant_id: &TenantId,
    config: PreSyncConfig,
    cli_config: &CliConfig,
    profile_path: &Path,
) -> Result<PreSyncResult> {
    if config.no_sync {
        CliService::warning("Pre-deploy sync skipped (--no-sync)");
        CliService::warning("Runtime files on the container will be LOST");
        return Ok(PreSyncResult::skipped());
    }

    CliService::section("Pre-Deploy Sync");
    display_destructive_warning();

    if !config.dry_run && !config.yes {
        let should_sync =
            confirm_optional("Sync files from cloud before deploying?", true, cli_config)?;

        if !should_sync {
            CliService::warning("Pre-deploy sync skipped by user");
            CliService::warning("Runtime files on the container will be LOST");
            return Ok(PreSyncResult::skipped());
        }
    }

    let (sync_config, api_client) = build_sync_config(
        tenant_id,
        config.dry_run,
        config.yes,
        cli_config,
        profile_path,
    )
    .await?;

    let services_path = std::path::PathBuf::from(&sync_config.services_path);

    if config.dry_run {
        let result = run_sync(sync_config, api_client).await;
        return handle_sync_result(result, true);
    }

    let service = FileSyncService::new(sync_config, api_client);

    let spinner = CliService::spinner("Downloading files from cloud...");
    let download = service
        .download_and_diff()
        .await
        .map_err(|e| anyhow::anyhow!("Download failed: {}", e))?;
    spinner.finish_and_clear();

    let spinner = CliService::spinner("Backing up local services...");
    let backup_path = FileSyncService::backup_services(&services_path)
        .map_err(|e| anyhow::anyhow!("Backup failed: {}", e))?;
    spinner.finish_and_clear();
    CliService::success(&format!("Backed up to {}", backup_path.display()));

    display_diff(&download.diff);

    if !download.diff.has_changes() {
        CliService::success("All files are already in sync");
        return Ok(PreSyncResult::success());
    }

    let changes = download.diff.added + download.diff.modified;
    if !config.yes {
        let prompt = format!(
            "Apply {} change{} from cloud? (backup saved)",
            changes,
            if changes == 1 { "" } else { "s" }
        );
        let should_apply = confirm_optional(&prompt, true, cli_config)?;

        if !should_apply {
            CliService::warning("Sync cancelled by user. Backup preserved.");
            return Ok(PreSyncResult::skipped());
        }
    }

    let changed_paths = download.diff.changed_paths();
    let count = FileSyncService::apply(&download.data, &services_path, Some(&changed_paths))
        .map_err(|e| anyhow::anyhow!("Apply failed: {}", e))?;

    CliService::success(&format!("Applied {} files from cloud", count));
    Ok(PreSyncResult::success())
}

async fn run_sync(
    config: systemprompt_sync::SyncConfig,
    api_client: SyncApiClient,
) -> systemprompt_sync::SyncResult<SyncOperationResult> {
    let spinner = CliService::spinner("Syncing files from cloud...");
    let service = FileSyncService::new(config, api_client);
    let result = service.sync().await;
    spinner.finish_and_clear();
    result
}
