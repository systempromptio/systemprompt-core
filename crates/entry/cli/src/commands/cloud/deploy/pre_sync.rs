use anyhow::{Context, Result};
use systemprompt_cloud::{get_cloud_paths, CloudPath, TenantStore};
use systemprompt_logging::CliService;
use systemprompt_models::{Profile, SecretsBootstrap};
use systemprompt_sync::{
    FileSyncService, SyncApiClient, SyncConfigBuilder, SyncDirection, SyncOperationResult,
};

use crate::cli_settings::CliConfig;
use crate::commands::cloud::tenant::get_credentials;
use crate::interactive::confirm_optional;
use crate::shared::project::ProjectRoot;

pub struct PreSyncConfig {
    pub no_sync: bool,
    pub yes: bool,
    pub dry_run: bool,
}

pub struct PreSyncResult {
    pub dry_run: bool,
}

impl PreSyncResult {
    const fn skipped() -> Self {
        Self { dry_run: false }
    }

    const fn dry_run() -> Self {
        Self { dry_run: true }
    }

    const fn success() -> Self {
        Self { dry_run: false }
    }
}

pub async fn execute(
    profile: &Profile,
    tenant_id: &str,
    config: PreSyncConfig,
    cli_config: &CliConfig,
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

    let Some((sync_config, api_client)) = build_sync_config(profile, tenant_id, config.dry_run)?
    else {
        CliService::warning("Skipping sync - runtime files on the container may be LOST");
        return Ok(PreSyncResult::skipped());
    };

    let result = run_sync(sync_config, api_client).await;

    handle_sync_result(result, config.dry_run)
}

fn build_sync_config(
    _profile: &Profile,
    tenant_id: &str,
    dry_run: bool,
) -> Result<Option<(systemprompt_sync::SyncConfig, SyncApiClient)>> {
    let secrets = SecretsBootstrap::get().context("Failed to load secrets")?;

    let sync_token = secrets.sync_token.clone();
    if sync_token.is_none() {
        CliService::warning("Sync token not configured in profile secrets");
        CliService::info("  Run: systemprompt cloud tenant rotate-sync-token");
        CliService::info("  Then recreate profile or update secrets.json manually");
        return Ok(None);
    }

    let creds = get_credentials()?;
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store.find_tenant(tenant_id);
    let hostname = tenant.and_then(|t| t.hostname.clone());

    if hostname.is_none() {
        CliService::warning("Hostname not configured for tenant");
        CliService::info("  Run: systemprompt cloud login");
        return Ok(None);
    }

    let project = ProjectRoot::discover().map_err(|e| anyhow::anyhow!("{}", e))?;
    let local_services_path = project.as_path().join("services");

    let sync_config = SyncConfigBuilder::new(
        tenant_id,
        &creds.api_url,
        &creds.api_token,
        local_services_path.to_string_lossy(),
    )
    .with_direction(SyncDirection::Pull)
    .with_dry_run(dry_run)
    .with_hostname(hostname)
    .with_sync_token(sync_token)
    .build();

    let api_client = SyncApiClient::new(&creds.api_url, &creds.api_token)
        .with_direct_sync(sync_config.hostname.clone(), sync_config.sync_token.clone());

    Ok(Some((sync_config, api_client)))
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

fn handle_sync_result(
    result: systemprompt_sync::SyncResult<SyncOperationResult>,
    dry_run: bool,
) -> Result<PreSyncResult> {
    match result {
        Ok(op) if op.success && dry_run => {
            display_dry_run_result(&op);
            Ok(PreSyncResult::dry_run())
        },
        Ok(op) if op.success => {
            CliService::success(&format!("Synced {} files from cloud", op.items_synced));
            Ok(PreSyncResult::success())
        },
        Ok(op) => {
            for err in &op.errors {
                CliService::error(&format!("Sync error: {}", err));
            }
            anyhow::bail!("Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).")
        },
        Err(e) => {
            CliService::error(&format!("Sync error: {}", e));
            anyhow::bail!("Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).")
        },
    }
}

fn display_destructive_warning() {
    CliService::warning("DESTRUCTIVE OPERATION");
    CliService::info("  Deploying replaces the running container.");
    CliService::info("  Runtime files (uploads, AI-generated images) not in your local build");
    CliService::info("  will be PERMANENTLY LOST unless synced first.");
    CliService::info("");
    CliService::info("  Database records are preserved.");
    CliService::info("");
}

fn display_dry_run_result(result: &SyncOperationResult) {
    CliService::section("Dry Run - Files to Sync");
    match &result.details {
        Some(details) => display_file_list(details),
        None => CliService::info(&format!("Would sync {} items", result.items_skipped)),
    }
}

fn display_file_list(details: &serde_json::Value) {
    let Some(files) = details.get("files").and_then(|f| f.as_array()) else {
        return;
    };

    CliService::info(&format!("Would sync {} files:", files.len()));

    for file in files.iter().take(20) {
        if let Some(path) = file.get("path").and_then(|p| p.as_str()) {
            CliService::info(&format!("  - {}", path));
        }
    }

    if files.len() > 20 {
        CliService::info(&format!("  ... and {} more", files.len() - 20));
    }
}
