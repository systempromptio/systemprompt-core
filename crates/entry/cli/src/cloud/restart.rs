use anyhow::{bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, TenantStore, TenantType};
use systemprompt_core_logging::CliService;

use super::tenant_ops::{get_credentials, resolve_tenant_id};

pub async fn execute(tenant_id: Option<String>) -> Result<()> {
    CliService::section("Restart Tenant");

    let resolved_tenant_id = resolve_tenant_id(tenant_id)?;

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_name = if let Ok(store) = TenantStore::load_from_path(&tenants_path) {
        if let Some(tenant) = store.find_tenant(&resolved_tenant_id) {
            if tenant.tenant_type != TenantType::Cloud {
                bail!("Restart is only available for cloud tenants");
            }
            tenant.name.clone()
        } else {
            resolved_tenant_id.clone()
        }
    } else {
        resolved_tenant_id.clone()
    };

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Restart tenant '{}'? This will cause a brief downtime.",
            tenant_name
        ))
        .default(false)
        .interact()?;

    if !confirm {
        CliService::info("Cancelled");
        return Ok(());
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner(&format!("Restarting tenant {}...", tenant_name));
    match client.restart_tenant(&resolved_tenant_id).await {
        Ok(response) => {
            spinner.finish_and_clear();
            CliService::success(&format!("Tenant restart initiated: {}", response.status));
            CliService::info("The tenant may take a few moments to become available again.");
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to restart tenant: {}", e);
        },
    }

    Ok(())
}
