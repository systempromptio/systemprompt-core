use anyhow::{Result, anyhow, bail};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use systemprompt_logging::CliService;

use super::select::select_tenant;
use crate::cli_settings::CliConfig;
use crate::cloud::types::TenantDetailOutput;
use crate::shared::CommandResult;

pub async fn show_tenant(
    id: Option<String>,
    config: &CliConfig,
) -> Result<CommandResult<TenantDetailOutput>> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        if !config.is_json_output() {
            CliService::warning(&format!("Failed to load tenant store: {}", e));
        }
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

    let output = TenantDetailOutput {
        id: tenant.id.clone(),
        name: tenant.name.clone(),
        tenant_type: format!("{:?}", tenant.tenant_type).to_lowercase(),
        app_id: tenant.app_id.clone(),
        hostname: tenant.hostname.clone(),
        region: tenant.region.clone(),
        has_database: tenant.has_database_url(),
    };

    if !config.is_json_output() {
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
    }

    Ok(CommandResult::card(output).with_title(format!("Tenant: {}", tenant.name)))
}
