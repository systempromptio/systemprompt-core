use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType,
};
use systemprompt_logging::CliService;

use super::docker::{
    drop_database_for_tenant, load_shared_config, save_shared_config, stop_shared_container,
};
use super::select::{get_credentials, select_tenant};
use crate::cli_settings::CliConfig;
use crate::cloud::tenant::{TenantCancelArgs, TenantDeleteArgs};

pub async fn list_tenants(config: &CliConfig) -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);

    let store = sync_and_load_tenants(&tenants_path).await;

    if store.tenants.is_empty() {
        CliService::section("Tenants");
        CliService::info("No tenants configured.");
        CliService::info("Run 'systemprompt cloud tenant create' (or 'just tenant') to create one.");
        return Ok(());
    }

    if !config.is_interactive() {
        CliService::section("Tenants");
        CliService::info("Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f");
        CliService::info("");
        for tenant in &store.tenants {
            let type_str = match tenant.tenant_type {
                TenantType::Local => "local",
                TenantType::Cloud => "cloud",
            };
            let db_status = if tenant.has_database_url() {
                "✓ db"
            } else {
                "✗ db"
            };
            CliService::info(&format!("{} ({}) [{}]", tenant.name, type_str, db_status));
        }
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
        CliService::info("Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f");
        CliService::info("");

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

pub async fn show_tenant(id: Option<String>, config: &CliConfig) -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    let tenant = match id {
        Some(ref id) => store
            .find_tenant(id)
            .ok_or_else(|| anyhow!("Tenant not found: {}", id))?,
        None if config.is_interactive() => {
            if store.tenants.is_empty() {
                bail!("No tenants configured.");
            }
            select_tenant(&store.tenants)?
        },
        None => bail!("--id is required in non-interactive mode for tenant show"),
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
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

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
    } else if tenant.uses_shared_container() {
        cleanup_shared_container_tenant(&tenant, config).await?;
    }

    store.tenants.retain(|t| t.id != tenant_id);
    store.save_to_path(&tenants_path)?;

    CliService::success(&format!("Deleted tenant: {}", tenant_id));

    Ok(())
}

async fn cleanup_shared_container_tenant(tenant: &StoredTenant, config: &CliConfig) -> Result<()> {
    let Some(ref db_name) = tenant.shared_container_db else {
        return Ok(());
    };

    let Some(mut shared_config) = load_shared_config()? else {
        CliService::warning("Shared container config not found, skipping database cleanup");
        return Ok(());
    };

    let spinner = CliService::spinner(&format!("Dropping database '{}'...", db_name));
    match drop_database_for_tenant(&shared_config.admin_password, shared_config.port, db_name).await
    {
        Ok(()) => {
            spinner.finish_and_clear();
            CliService::success(&format!("Database '{}' dropped", db_name));
        },
        Err(e) => {
            spinner.finish_and_clear();
            CliService::warning(&format!("Failed to drop database '{}': {}", db_name, e));
        },
    }

    shared_config.remove_tenant(&tenant.id);
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

pub async fn edit_tenant(id: Option<String>, config: &CliConfig) -> Result<()> {
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

async fn sync_and_load_tenants(tenants_path: &std::path::Path) -> TenantStore {
    let mut local_store =
        TenantStore::load_from_path(tenants_path).unwrap_or_else(|_| TenantStore::default());

    let Ok(creds) = get_credentials() else {
        return local_store;
    };

    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let cloud_tenant_infos = match client.get_user().await {
        Ok(response) => response.tenants,
        Err(e) => {
            CliService::warning(&format!("Failed to sync cloud tenants: {}", e));
            return local_store;
        },
    };

    for cloud_info in &cloud_tenant_infos {
        if let Some(existing) = local_store
            .tenants
            .iter_mut()
            .find(|t| t.id == cloud_info.id)
        {
            existing.update_from_tenant_info(cloud_info);
        } else {
            local_store
                .tenants
                .push(StoredTenant::from_tenant_info(cloud_info));
        }
    }

    local_store.synced_at = Utc::now();

    if let Err(e) = local_store.save_to_path(tenants_path) {
        CliService::warning(&format!("Failed to save synced tenants: {}", e));
    }

    local_store
}

pub async fn cancel_subscription(args: TenantCancelArgs, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        bail!(
            "Subscription cancellation requires interactive mode for safety.\nThis is an \
             irreversible operation that destroys all data."
        );
    }

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store =
        TenantStore::load_from_path(&tenants_path).unwrap_or_else(|_| TenantStore::default());

    let cloud_tenants: Vec<&StoredTenant> = store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == TenantType::Cloud)
        .collect();

    if cloud_tenants.is_empty() {
        bail!("No cloud tenants found. Only cloud tenants have subscriptions.");
    }

    let tenant = if let Some(ref id) = args.id {
        store
            .tenants
            .iter()
            .find(|t| t.id == *id && t.tenant_type == TenantType::Cloud)
            .ok_or_else(|| anyhow!("Cloud tenant not found: {}", id))?
    } else {
        let options: Vec<String> = cloud_tenants
            .iter()
            .map(|t| format!("{} ({})", t.name, t.id))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select cloud tenant to cancel")
            .items(&options)
            .default(0)
            .interact()?;

        cloud_tenants[selection]
    };

    CliService::section("⚠️  CANCEL SUBSCRIPTION");
    CliService::error("THIS ACTION IS IRREVERSIBLE");
    CliService::info("");
    CliService::info("This will:");
    CliService::info("  • Cancel your subscription immediately");
    CliService::info("  • Stop and destroy the Fly.io machine");
    CliService::info("  • Delete ALL data in the database");
    CliService::info("  • Remove all deployed code and configuration");
    CliService::info("");
    CliService::warning("Your data CANNOT be recovered after this operation.");
    CliService::info("");

    CliService::key_value("Tenant", &tenant.name);
    CliService::key_value("ID", &tenant.id);
    if let Some(ref hostname) = tenant.hostname {
        CliService::key_value("URL", &format!("https://{}", hostname));
    }
    CliService::info("");

    let confirmation: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Type '{}' to confirm cancellation", tenant.name))
        .interact_text()?;

    if confirmation != tenant.name {
        CliService::info("Cancellation aborted. Tenant name did not match.");
        return Ok(());
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner("Cancelling subscription...");
    client.cancel_subscription(&tenant.id).await?;
    spinner.finish_and_clear();

    CliService::success("Subscription cancelled");
    CliService::info("Your tenant will be suspended and all data will be destroyed.");
    CliService::info("You will not be charged for future billing periods.");
    CliService::info("");
    CliService::info(
        "Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f",
    );

    Ok(())
}
