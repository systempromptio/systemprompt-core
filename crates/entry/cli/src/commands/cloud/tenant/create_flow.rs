//! Interactive `cloud tenant create` workflow.
//!
//! Prompts for the database source, creates the tenant via the matching
//! constructor, persists it to the tenant store, and renders the result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_cloud::{CloudPath, StoredTenant, TenantStore, get_cloud_paths};
use systemprompt_logging::CliService;

use super::{create_external_tenant, create_local_tenant};
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;

pub(super) async fn tenant_create(prompter: &dyn Prompter, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Tenant creation requires interactive mode.\nUse specific tenant type commands in \
             non-interactive mode (not yet implemented)."
        ));
    }

    CliService::section("Create Tenant");

    let tenant = create_local_or_external_tenant(prompter).await?;

    persist_tenant(&tenant)?;
    render_created_tenant(&tenant);

    Ok(())
}

async fn create_local_or_external_tenant(prompter: &dyn Prompter) -> Result<StoredTenant> {
    let db_options = vec![
        "Docker (creates PostgreSQL container automatically)".to_owned(),
        "External PostgreSQL (use your own database)".to_owned(),
    ];

    let db_selection = prompter.select("Database source", &db_options)?;

    match db_selection {
        0 => create_local_tenant(prompter).await,
        _ => create_external_tenant(prompter).await,
    }
}

fn persist_tenant(tenant: &StoredTenant) -> Result<()> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    if let Some(existing) = store.tenants.iter_mut().find(|t| t.id == tenant.id) {
        *existing = tenant.clone();
    } else {
        store.tenants.push(tenant.clone());
    }
    store.save_to_path(&tenants_path)?;

    Ok(())
}

fn render_created_tenant(tenant: &StoredTenant) {
    CliService::success("Tenant created");
    CliService::key_value("ID", tenant.id.as_str());
    CliService::key_value("Name", &tenant.name);
    CliService::key_value("Type", &format!("{:?}", tenant.tenant_type));

    if let Some(ref url) = tenant.database_url
        && !url.is_empty()
    {
        CliService::key_value("Database URL", url);
    }
}
