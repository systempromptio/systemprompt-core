use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_cloud::{CloudApiClient, CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_sync::{SyncApiClient, SyncConfigBuilder, SyncDirection};

use crate::cli_settings::CliConfig;
use crate::commands::cloud::tenant::get_credentials;
use crate::interactive::confirm_optional;
use crate::shared::project::ProjectRoot;

pub async fn build_sync_config(
    tenant_id: &TenantId,
    dry_run: bool,
    yes: bool,
    cli_config: &CliConfig,
    profile_path: &Path,
) -> Result<(systemprompt_sync::SyncConfig, SyncApiClient)> {
    let secrets = SecretsBootstrap::get().context("Failed to load secrets")?;
    let creds = get_credentials()?;

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store.find_tenant(tenant_id.as_str());
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
        tenant_id.clone(),
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
    tenant_id: &TenantId,
    yes: bool,
    cli_config: &CliConfig,
    profile_path: &Path,
    tenant_store: &mut TenantStore,
) -> Result<Option<String>> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let creds = get_credentials()?;

    CliService::section("Sync Token Setup");
    CliService::warning("Sync token not configured in profile secrets");

    let stored_token = tenant_store
        .find_tenant(tenant_id.as_str())
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

        if let Some(tenant) = tenant_store
            .tenants
            .iter_mut()
            .find(|t| t.id == tenant_id.as_str())
        {
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
