//! `cloud tenant delete` command.
//!
//! Removes a tenant from the store, cancelling its cloud subscription via the
//! API or dropping its shared-container database for local tenants, and tears
//! down the shared `PostgreSQL` container once the last local tenant is gone.

use anyhow::{Result, anyhow, bail};
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use systemprompt_cloud::{
    CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;

use super::docker::{
    drop_database_for_tenant, load_shared_config, save_shared_config, stop_shared_container,
};
use super::select::{get_credentials, select_tenant};
use crate::cli_settings::CliConfig;
use crate::cloud::tenant::TenantDeleteArgs;
use crate::shared::{CommandOutput, SuccessOutput};

pub async fn delete_tenant(args: TenantDeleteArgs, config: &CliConfig) -> Result<CommandOutput> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        if !config.is_json_output() {
            CliService::warning(&format!("Failed to load tenant store: {}", e));
        }
        TenantStore::default()
    });

    let tenant_id = resolve_delete_target(args.id, &store, config)?;

    let tenant = store
        .tenants
        .iter()
        .find(|t| t.id == tenant_id.as_str())
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id.as_str()))?
        .clone();

    let is_cloud = tenant.tenant_type == TenantType::Cloud;

    if !args.yes && !confirm_delete(&tenant, is_cloud, config)? {
        let output = SuccessOutput::new("Cancelled");
        if !config.is_json_output() {
            CliService::info("Cancelled");
        }
        return Ok(CommandOutput::text_titled("Delete Tenant", output.message));
    }

    if is_cloud {
        delete_cloud_tenant(&tenant_id, config).await?;
    } else if tenant.uses_shared_container() {
        cleanup_shared_container_tenant(&tenant, config)?;
    }

    store.tenants.retain(|t| t.id != tenant_id.as_str());
    store.save_to_path(&tenants_path)?;

    let output = SuccessOutput::new(format!("Deleted tenant: {}", tenant_id.as_str()));

    if !config.is_json_output() {
        CliService::success(&format!("Deleted tenant: {}", tenant_id.as_str()));
    }

    Ok(CommandOutput::text_titled("Delete Tenant", output.message))
}

fn resolve_delete_target(
    id: Option<String>,
    store: &TenantStore,
    config: &CliConfig,
) -> Result<TenantId> {
    if let Some(id) = id {
        return Ok(TenantId::new(id));
    }
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "--id is required in non-interactive mode for tenant delete"
        ));
    }
    if store.tenants.is_empty() {
        bail!("No tenants configured.");
    }
    Ok(TenantId::new(select_tenant(&store.tenants)?.id.clone()))
}

fn confirm_delete(tenant: &StoredTenant, is_cloud: bool, config: &CliConfig) -> Result<bool> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "--yes is required in non-interactive mode for tenant delete"
        ));
    }

    let prompt = if is_cloud {
        format!(
            "Delete cloud tenant '{}'? This will cancel your subscription and delete all data.",
            tenant.name
        )
    } else {
        format!("Delete tenant '{}'?", tenant.name)
    };

    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(false)
        .interact()?)
}

async fn delete_cloud_tenant(tenant_id: &TenantId, config: &CliConfig) -> Result<()> {
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    if config.is_json_output() {
        client.delete_tenant(tenant_id).await?;
    } else {
        let spinner = CliService::spinner("Deleting cloud tenant...");
        client.delete_tenant(tenant_id).await?;
        spinner.finish_and_clear();
    }

    Ok(())
}

fn cleanup_shared_container_tenant(tenant: &StoredTenant, config: &CliConfig) -> Result<()> {
    let Some(ref db_name) = tenant.shared_container_db else {
        return Ok(());
    };

    let Some(mut shared_config) = load_shared_config()? else {
        CliService::warning("Shared container config not found, skipping database cleanup");
        return Ok(());
    };

    let spinner = CliService::spinner(&format!("Dropping database '{}'...", db_name));
    match drop_database_for_tenant(&shared_config.admin_password, shared_config.port, db_name) {
        Ok(()) => {
            spinner.finish_and_clear();
            CliService::success(&format!("Database '{}' dropped", db_name));
        },
        Err(e) => {
            spinner.finish_and_clear();
            CliService::warning(&format!("Failed to drop database '{}': {}", db_name, e));
        },
    }

    shared_config.remove_tenant(tenant.id.as_str());
    save_shared_config(&shared_config)?;

    if shared_config.tenant_databases.is_empty() {
        let should_remove = if config.is_interactive() {
            Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("No local tenants remain. Remove shared PostgreSQL container?")
                .default(true)
                .interact()?
        } else {
            false
        };

        if should_remove {
            stop_shared_container()?;
        } else {
            CliService::info(
                "Shared container kept. Remove manually with 'docker compose -f \
                 .systemprompt/docker/shared.yaml down -v'",
            );
        }
    }

    Ok(())
}
