use anyhow::{bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, TenantStore, TenantType};
use systemprompt_logging::CliService;

use super::tenant::{get_credentials, resolve_tenant_id};
use super::types::RestartOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;

pub async fn execute(
    tenant_id: Option<String>,
    yes: bool,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    if !config.is_json_output() {
        CliService::section("Restart Tenant");
    }

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

    if !yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for restart"
            ));
        }

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Restart tenant '{}'? This will cause a brief downtime.",
                tenant_name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            if !config.is_json_output() {
                CliService::info("Cancelled");
            }
            let output = RestartOutput {
                tenant_name: tenant_name.clone(),
                status: "cancelled".to_string(),
            };
            return Ok(CommandResult::text(output).with_title("Restart Tenant"));
        }
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let response = if config.is_json_output() {
        client.restart_tenant(&resolved_tenant_id).await?
    } else {
        let spinner = CliService::spinner(&format!("Restarting tenant {}...", tenant_name));
        match client.restart_tenant(&resolved_tenant_id).await {
            Ok(response) => {
                spinner.finish_and_clear();
                response
            },
            Err(e) => {
                spinner.finish_and_clear();
                bail!("Failed to restart tenant: {}", e);
            },
        }
    };

    let output = RestartOutput {
        tenant_name: tenant_name.clone(),
        status: response.status.clone(),
    };

    if !config.is_json_output() {
        CliService::success(&format!("Tenant restart initiated: {}", response.status));
        CliService::info("The tenant may take a few moments to become available again.");
    }

    Ok(CommandResult::text(output).with_title("Restart Tenant"))
}
