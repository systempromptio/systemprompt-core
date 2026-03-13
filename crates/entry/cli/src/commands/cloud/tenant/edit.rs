use anyhow::{Result, anyhow, bail};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use systemprompt_cloud::{CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths};
use systemprompt_logging::CliService;

use super::select::select_tenant;
use crate::cli_settings::CliConfig;
use crate::cloud::types::TenantDetailOutput;
use crate::shared::CommandResult;

pub async fn edit_tenant(
    id: Option<String>,
    config: &CliConfig,
) -> Result<CommandResult<TenantDetailOutput>> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Tenant edit requires interactive mode.\nUse specific commands to modify tenant \
             settings in non-interactive mode."
        ));
    }

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    let tenant_id = if let Some(id) = id {
        id
    } else {
        if store.tenants.is_empty() {
            bail!("No tenants configured.");
        }
        select_tenant(&store.tenants)?.id.clone()
    };

    let tenant = store
        .tenants
        .iter_mut()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id))?;

    CliService::section(&format!("Edit Tenant: {}", tenant.name));

    let new_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant name")
        .default(tenant.name.clone())
        .interact_text()?;

    if new_name.is_empty() {
        bail!("Tenant name cannot be empty");
    }
    tenant.name.clone_from(&new_name);

    if tenant.tenant_type == TenantType::Local {
        edit_local_tenant_database(tenant)?;
    }

    if tenant.tenant_type == TenantType::Cloud {
        display_readonly_cloud_fields(tenant);
    }

    let output = TenantDetailOutput {
        id: tenant.id.clone(),
        name: tenant.name.clone(),
        tenant_type: format!("{:?}", tenant.tenant_type).to_lowercase(),
        app_id: tenant.app_id.clone(),
        hostname: tenant.hostname.clone(),
        region: tenant.region.clone(),
        has_database: tenant.has_database_url(),
    };

    store.save_to_path(&tenants_path)?;
    CliService::success(&format!("Tenant '{}' updated", new_name));

    Ok(CommandResult::card(output)
        .with_title(format!("Tenant: {}", new_name))
        .with_skip_render())
}

fn edit_local_tenant_database(tenant: &mut StoredTenant) -> Result<()> {
    if let Some(current_url) = tenant.database_url.clone() {
        let edit_db = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Edit database URL?")
            .default(false)
            .interact()?;

        if edit_db {
            let new_url: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Database URL")
                .default(current_url)
                .interact_text()?;
            tenant.database_url = if new_url.is_empty() {
                None
            } else {
                Some(new_url)
            };
        }
    }
    Ok(())
}

fn display_readonly_cloud_fields(tenant: &StoredTenant) {
    if let Some(ref region) = tenant.region {
        CliService::info(&format!("Region: {} (cannot be changed)", region));
    }
    if let Some(ref hostname) = tenant.hostname {
        CliService::info(&format!("Hostname: {} (cannot be changed)", hostname));
    }
}
