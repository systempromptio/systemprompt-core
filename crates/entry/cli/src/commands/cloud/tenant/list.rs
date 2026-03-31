use anyhow::Result;
use chrono::Utc;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_cloud::{
    CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_logging::CliService;

use super::select::get_credentials;
use crate::cli_settings::CliConfig;
use crate::cloud::types::{TenantListOutput, TenantSummary};
use crate::shared::CommandResult;

pub async fn list_tenants(config: &CliConfig) -> Result<CommandResult<TenantListOutput>> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);

    let store = sync_and_load_tenants(&tenants_path).await;

    let summaries: Vec<TenantSummary> = store
        .tenants
        .iter()
        .map(|t| TenantSummary {
            id: t.id.clone(),
            name: t.name.clone(),
            tenant_type: format!("{:?}", t.tenant_type).to_lowercase(),
            has_database: t.has_database_url(),
        })
        .collect();

    let output = TenantListOutput {
        total: summaries.len(),
        tenants: summaries,
    };

    if store.tenants.is_empty() {
        if !config.is_json_output() {
            CliService::section("Tenants");
            CliService::info("No tenants configured.");
            CliService::info(
                "Run 'systemprompt cloud tenant create' (or 'just tenant') to create one.",
            );
        }
        return Ok(CommandResult::table(output)
            .with_title("Tenants")
            .with_columns(vec![
                "id".to_string(),
                "name".to_string(),
                "tenant_type".to_string(),
                "has_database".to_string(),
            ]));
    }

    if !config.is_json_output() {
        if config.is_interactive() {
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
        } else {
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
        }
    }

    Ok(CommandResult::table(output)
        .with_title("Tenants")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "tenant_type".to_string(),
            "has_database".to_string(),
        ]))
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

async fn sync_and_load_tenants(tenants_path: &std::path::Path) -> TenantStore {
    let mut local_store =
        TenantStore::load_from_path(tenants_path).unwrap_or_else(|_| TenantStore::default());

    let Ok(creds) = get_credentials() else {
        return local_store;
    };

    let Ok(client) = CloudApiClient::new(&creds.api_url, &creds.api_token) else {
        return local_store;
    };

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
