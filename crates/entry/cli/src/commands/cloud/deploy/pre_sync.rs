use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_cloud::{CloudApiClient, CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_logging::CliService;
use systemprompt_models::SecretsBootstrap;
use systemprompt_sync::{
    FileDiffStatus, FileSyncService, SyncApiClient, SyncConfigBuilder, SyncDiffResult,
    SyncDirection, SyncOperationResult,
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
    tenant_id: &str,
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

    // Step 1: Download and compute diff
    let service = FileSyncService::new(sync_config, api_client);

    let spinner = CliService::spinner("Downloading files from cloud...");
    let download = service
        .download_and_diff()
        .await
        .map_err(|e| anyhow::anyhow!("Download failed: {}", e))?;
    spinner.finish_and_clear();

    // Step 2: Backup local services to zip
    let spinner = CliService::spinner("Backing up local services...");
    let backup_path = FileSyncService::backup_services(&services_path)
        .map_err(|e| anyhow::anyhow!("Backup failed: {}", e))?;
    spinner.finish_and_clear();
    CliService::success(&format!("Backed up to {}", backup_path.display()));

    // Step 3: Display diff
    display_diff(&download.diff);

    // Step 4: If no changes, we're done
    if !download.diff.has_changes() {
        CliService::success("All files are already in sync");
        return Ok(PreSyncResult::success());
    }

    // Step 5: Interactive confirmation (skip with --yes)
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

    // Step 6: Apply only changed files
    let changed_paths = download.diff.changed_paths();
    let count = FileSyncService::apply(&download.data, &services_path, Some(&changed_paths))
        .map_err(|e| anyhow::anyhow!("Apply failed: {}", e))?;

    CliService::success(&format!("Applied {} files from cloud", count));
    Ok(PreSyncResult::success())
}

fn display_diff(diff: &SyncDiffResult) {
    CliService::section("Cloud Sync Diff");

    if diff.added > 0 {
        CliService::info(&format!("  Added ({}):", diff.added));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Added {
                CliService::info(&format!("    + {}", entry.path));
            }
        }
    }

    if diff.modified > 0 {
        CliService::info(&format!("  Modified ({}):", diff.modified));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Modified {
                CliService::info(&format!("    ~ {}", entry.path));
            }
        }
    }

    if diff.deleted > 0 {
        CliService::info(&format!("  Deleted from cloud ({}):", diff.deleted));
        for entry in &diff.entries {
            if entry.status == FileDiffStatus::Deleted {
                CliService::info(&format!("    - {}", entry.path));
            }
        }
    }

    if diff.unchanged > 0 {
        CliService::info(&format!("  Unchanged ({} files identical)", diff.unchanged));
    }
}

async fn build_sync_config(
    tenant_id: &str,
    dry_run: bool,
    yes: bool,
    cli_config: &CliConfig,
    profile_path: &Path,
) -> Result<(systemprompt_sync::SyncConfig, SyncApiClient)> {
    let secrets = SecretsBootstrap::get().context("Failed to load secrets")?;
    let creds = get_credentials()?;

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store.find_tenant(tenant_id);
    let hostname = tenant.and_then(|t| t.hostname.clone());

    if hostname.is_none() {
        anyhow::bail!("Hostname not configured for tenant.\nRun: systemprompt cloud login");
    }

    let mut sync_token = secrets.sync_token.clone();

    if sync_token.is_none() {
        sync_token =
            setup_sync_token(tenant_id, yes, cli_config, profile_path, &mut tenant_store).await?;
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

    let api_client = SyncApiClient::new(&creds.api_url, &creds.api_token)?
        .with_direct_sync(sync_config.hostname.clone(), sync_config.sync_token.clone());

    Ok((sync_config, api_client))
}

async fn setup_sync_token(
    tenant_id: &str,
    yes: bool,
    cli_config: &CliConfig,
    profile_path: &Path,
    tenant_store: &mut TenantStore,
) -> Result<Option<String>> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let creds = get_credentials()?;

    CliService::section("Sync Token Setup");
    CliService::warning("Sync token not configured in profile secrets");

    let stored_token = tenant_store
        .find_tenant(tenant_id)
        .and_then(|t| t.sync_token.clone());

    let token = if let Some(token) = stored_token {
        CliService::info("Found existing sync token in local tenant store");
        token
    } else {
        CliService::info("No sync token found - generating a new one...");

        if !yes {
            let should_generate = confirm_optional(
                "Generate a new sync token for file synchronization?",
                true,
                cli_config,
            )?;
            if !should_generate {
                anyhow::bail!(
                    "Sync token required for pre-deploy sync.\nUse --no-sync to skip sync \
                     explicitly."
                );
            }
        }

        let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;
        let spinner = CliService::spinner("Generating sync token...");
        let response = client.rotate_sync_token(tenant_id).await?;
        spinner.finish_and_clear();

        let token = response.sync_token;
        CliService::success("Sync token generated");

        if let Some(tenant) = tenant_store.tenants.iter_mut().find(|t| t.id == tenant_id) {
            tenant.sync_token = Some(token.clone());
        }
        tenant_store.save_to_path(&tenants_path)?;

        token
    };

    save_sync_token_to_secrets(profile_path, &token)?;
    CliService::success("Sync token saved to profile secrets");

    Ok(Some(token))
}

fn save_sync_token_to_secrets(profile_path: &Path, token: &str) -> Result<()> {
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        anyhow::bail!(
            "Secrets file not found: {}\nRun 'systemprompt cloud profile create' first.",
            secrets_path.display()
        );
    }

    let content = std::fs::read_to_string(&secrets_path)
        .with_context(|| format!("Failed to read {}", secrets_path.display()))?;

    let mut secrets: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse secrets.json")?;

    secrets["sync_token"] = serde_json::Value::String(token.to_string());

    let updated = serde_json::to_string_pretty(&secrets).context("Failed to serialize secrets")?;

    std::fs::write(&secrets_path, updated)
        .with_context(|| format!("Failed to write {}", secrets_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&secrets_path, permissions)
            .with_context(|| format!("Failed to set permissions on {}", secrets_path.display()))?;
    }

    Ok(())
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
