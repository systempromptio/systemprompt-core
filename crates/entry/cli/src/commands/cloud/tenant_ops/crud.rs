use anyhow::{anyhow, bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType,
};
use systemprompt_core_logging::CliService;

use super::select::{get_credentials, select_tenant};
use crate::cli_settings::CliConfig;
use crate::cloud::tenant::TenantDeleteArgs;

pub async fn list_tenants() -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

    if store.tenants.is_empty() {
        CliService::section("Tenants");
        CliService::info("No tenants configured.");
        CliService::info("Run 'systemprompt cloud tenant create' to create one.");
        return Ok(());
    }

    let options: Vec<String> = store
        .tenants
        .iter()
        .map(|t| {
            let type_str = match t.tenant_type {
                TenantType::Local => "local",
                TenantType::Cloud => "cloud",
            };
            let db_status = if t.has_database_url() {
                "✓ db"
            } else {
                "✗ db"
            };
            format!("{} ({}) [{}]", t.name, type_str, db_status)
        })
        .chain(std::iter::once("Back".to_string()))
        .collect();

    loop {
        CliService::section("Tenants");

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select tenant")
            .items(&options)
            .default(0)
            .interact()?;

        if selection == store.tenants.len() {
            break;
        }

        display_tenant_details(&store.tenants[selection]);
    }

    Ok(())
}

fn display_tenant_details(tenant: &StoredTenant) {
    CliService::section(&format!("Tenant: {}", tenant.name));
    CliService::key_value("ID", &tenant.id);
    CliService::key_value("Type", &format!("{:?}", tenant.tenant_type));

    if let Some(ref app_id) = tenant.app_id {
        CliService::key_value("App ID", app_id);
    }

    if let Some(ref hostname) = tenant.hostname {
        CliService::key_value("Hostname", hostname);
    }

    if let Some(ref region) = tenant.region {
        CliService::key_value("Region", region);
    }

    CliService::key_value(
        "Database",
        if tenant.has_database_url() {
            "configured"
        } else {
            "not configured"
        },
    );
}

pub async fn show_tenant(id: Option<String>) -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

    let tenant = if let Some(ref id) = id {
        store
            .find_tenant(id)
            .ok_or_else(|| anyhow!("Tenant not found: {}", id))?
    } else {
        if store.tenants.is_empty() {
            bail!("No tenants configured.");
        }
        select_tenant(&store.tenants)?
    };

    CliService::section(&format!("Tenant: {}", tenant.name));
    CliService::key_value("ID", &tenant.id);
    CliService::key_value("Type", &format!("{:?}", tenant.tenant_type));

    if let Some(ref app_id) = tenant.app_id {
        CliService::key_value("App ID", app_id);
    }

    if let Some(ref hostname) = tenant.hostname {
        CliService::key_value("Hostname", hostname);
    }

    if let Some(ref region) = tenant.region {
        CliService::key_value("Region", region);
    }

    if tenant.has_database_url() {
        CliService::key_value("Database", "configured");
    } else {
        CliService::key_value("Database", "not configured");
    }

    Ok(())
}

pub async fn delete_tenant(args: TenantDeleteArgs, config: &CliConfig) -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

    let tenant_id = if let Some(id) = args.id {
        id
    } else {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--id is required in non-interactive mode for tenant delete"
            ));
        }
        if store.tenants.is_empty() {
            bail!("No tenants configured.");
        }
        select_tenant(&store.tenants)?.id.clone()
    };

    let tenant = store
        .tenants
        .iter()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id))?
        .clone();

    let is_cloud = tenant.tenant_type == TenantType::Cloud;

    if !args.yes {
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

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(false)
            .interact()?;

        if !confirm {
            CliService::info("Cancelled");
            return Ok(());
        }
    }

    if is_cloud {
        let creds = get_credentials()?;
        let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

        let spinner = CliService::spinner("Deleting cloud tenant...");
        client.delete_tenant(&tenant_id).await?;
        spinner.finish_and_clear();
    }

    store.tenants.retain(|t| t.id != tenant_id);
    store.save_to_path(&tenants_path)?;

    CliService::success(&format!("Deleted tenant: {}", tenant_id));

    Ok(())
}

pub async fn edit_tenant(id: Option<String>, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Tenant edit requires interactive mode.\n\
             Use specific commands to modify tenant settings in non-interactive mode."
        ));
    }

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

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

    store.save_to_path(&tenants_path)?;
    CliService::success(&format!("Tenant '{}' updated", new_name));

    Ok(())
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
