//! `cloud restart` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use systemprompt_cloud::{CloudApiClient, CloudPath, TenantStore, TenantType, get_cloud_paths};
use systemprompt_logging::CliService;

use super::tenant::{get_credentials, resolve_tenant_id};
use super::types::RestartOutput;
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub(super) async fn execute(
    tenant: Option<String>,
    yes: bool,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_json_output() {
        CliService::section("Restart Tenant");
    }

    let resolved_tenant_id = resolve_tenant_id(tenant)?;

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_name = if let Ok(store) = TenantStore::load_from_path(&tenants_path) {
        if let Some(tenant) = store.find_tenant(&resolved_tenant_id) {
            if tenant.tenant_type != TenantType::Cloud {
                bail!("Restart is only available for cloud tenants");
            }
            tenant.name.clone()
        } else {
            resolved_tenant_id.to_string()
        }
    } else {
        resolved_tenant_id.to_string()
    };

    if !yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for restart"
            ));
        }

        let confirm = prompter.confirm(
            &format!(
                "Restart tenant '{}'? This will cause a brief downtime.",
                tenant_name
            ),
            false,
        )?;

        if !confirm {
            if !config.is_json_output() {
                CliService::info("Cancelled");
            }
            let output = RestartOutput {
                tenant_name: tenant_name.clone(),
                status: "cancelled".to_owned(),
            };
            return Ok(CommandOutput::card_value("Restart Tenant", &output));
        }
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;

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

    Ok(CommandOutput::card_value("Restart Tenant", &output))
}
