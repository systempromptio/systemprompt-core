//! `cloud tenant list` command.
//!
//! Reconciles the local tenant store with the cloud account, then renders the
//! merged set as a [`TenantListOutput`] table, with an interactive drill-down
//! into per-tenant details when running interactively.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use chrono::Utc;
use systemprompt_cloud::{
    CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_logging::CliService;

use super::select::get_credentials;
use crate::cli_settings::CliConfig;
use crate::cloud::types::{TenantListOutput, TenantSummary};
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub async fn list_tenants(prompter: &dyn Prompter, config: &CliConfig) -> Result<CommandOutput> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);

    let store = sync_and_load_tenants(&tenants_path).await;

    let summaries: Vec<TenantSummary> = store
        .tenants
        .iter()
        .map(|t| TenantSummary {
            id: t.id.as_str().to_owned(),
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
        return Ok(CommandOutput::table_of(
            vec!["id", "name", "tenant_type", "has_database"],
            &output.tenants,
        )
        .with_title("Tenants"));
    }

    if !config.is_json_output() {
        if config.is_interactive() {
            run_tenant_picker(prompter, &store)?;
        } else {
            render_tenant_lines(&store);
        }
    }

    Ok(CommandOutput::table_of(
        vec!["id", "name", "tenant_type", "has_database"],
        &output.tenants,
    )
    .with_title("Tenants"))
}

fn tenant_label(tenant: &StoredTenant) -> String {
    let type_str = match tenant.tenant_type {
        TenantType::Local => "local",
        TenantType::Cloud => "cloud",
    };
    let db_status = if tenant.has_database_url() {
        "✓ db"
    } else {
        "✗ db"
    };
    format!("{} ({}) [{}]", tenant.name, type_str, db_status)
}

fn run_tenant_picker(prompter: &dyn Prompter, store: &TenantStore) -> Result<()> {
    let options: Vec<String> = store
        .tenants
        .iter()
        .map(tenant_label)
        .chain(std::iter::once("Back".to_owned()))
        .collect();

    loop {
        CliService::section("Tenants");
        CliService::info(
            "Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f",
        );
        CliService::info("");

        let selection = prompter.select("Select tenant", &options)?;

        if selection == store.tenants.len() {
            break;
        }

        display_tenant_details(&store.tenants[selection]);
    }

    Ok(())
}

fn render_tenant_lines(store: &TenantStore) {
    CliService::section("Tenants");
    CliService::info(
        "Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f",
    );
    CliService::info("");
    for tenant in &store.tenants {
        CliService::info(&tenant_label(tenant));
    }
}

fn display_tenant_details(tenant: &StoredTenant) {
    CliService::section(&format!("Tenant: {}", tenant.name));
    CliService::key_value("ID", tenant.id.as_str());
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

    let Ok(client) = CloudApiClient::new(&creds.api_url, creds.api_token.as_str()) else {
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
            .find(|t| t.id.as_str() == cloud_info.id)
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
