use anyhow::{anyhow, bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, TenantStore, TenantType};
use systemprompt_logging::CliService;

use super::select::{get_credentials, select_tenant};
use crate::cli_settings::CliConfig;
use crate::cloud::types::{RotateCredentialsOutput, RotateSyncTokenOutput};
use crate::shared::CommandResult;

pub async fn rotate_credentials(
    id: Option<String>,
    skip_confirm: bool,
    config: &CliConfig,
) -> Result<CommandResult<RotateCredentialsOutput>> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        if !config.is_json_output() {
            CliService::warning(&format!("Failed to load tenant store: {}", e));
        }
        TenantStore::default()
    });

    let tenant_id = if let Some(id) = id {
        id
    } else {
        if store.tenants.is_empty() {
            bail!("No tenants configured.");
        }
        if skip_confirm {
            bail!("Tenant ID required in non-interactive mode");
        }
        select_tenant(&store.tenants)?.id.clone()
    };

    let tenant = store
        .tenants
        .iter()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id))?;

    if tenant.tenant_type != TenantType::Cloud {
        bail!("Credential rotation is only available for cloud tenants");
    }

    if !skip_confirm {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Rotate database credentials for '{}'? This will generate a new password.",
                tenant.name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            if !config.is_json_output() {
                CliService::info("Cancelled");
            }
            let output = RotateCredentialsOutput {
                tenant_id: tenant_id.clone(),
                status: "cancelled".to_string(),
                internal_database_url: String::new(),
                external_database_url: String::new(),
            };
            return Ok(CommandResult::card(output).with_title("Rotate Credentials"));
        }
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let response = if config.is_json_output() {
        client.rotate_credentials(&tenant_id).await?
    } else {
        let spinner = CliService::spinner("Rotating database credentials...");
        let resp = client.rotate_credentials(&tenant_id).await?;
        spinner.finish_and_clear();
        resp
    };

    let tenant = store
        .tenants
        .iter_mut()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found after rotation"))?;

    tenant.internal_database_url = Some(response.internal_database_url.clone());
    if tenant.external_db_access {
        tenant.database_url = Some(response.external_database_url.clone());
    }

    store.save_to_path(&tenants_path)?;

    let output = RotateCredentialsOutput {
        tenant_id: tenant_id.clone(),
        status: response.status.clone(),
        internal_database_url: response.internal_database_url.clone(),
        external_database_url: response.external_database_url.clone(),
    };

    if !config.is_json_output() {
        CliService::success("Database credentials rotated");
        CliService::key_value("Status", &response.status);

        CliService::section("New Database Connection");
        CliService::key_value("Internal URL", &response.internal_database_url);
        CliService::key_value("External URL", &response.external_database_url);
    }

    Ok(CommandResult::card(output).with_title("Rotate Credentials"))
}

pub async fn rotate_sync_token(
    id: Option<String>,
    skip_confirm: bool,
    config: &CliConfig,
) -> Result<CommandResult<RotateSyncTokenOutput>> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        if !config.is_json_output() {
            CliService::warning(&format!("Failed to load tenant store: {}", e));
        }
        TenantStore::default()
    });

    let tenant_id = if let Some(id) = id {
        id
    } else {
        if store.tenants.is_empty() {
            bail!("No tenants configured.");
        }
        if skip_confirm {
            bail!("Tenant ID required in non-interactive mode");
        }
        select_tenant(&store.tenants)?.id.clone()
    };

    let tenant = store
        .tenants
        .iter()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id))?;

    if tenant.tenant_type != TenantType::Cloud {
        bail!("Sync token rotation is only available for cloud tenants");
    }

    if !skip_confirm {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Rotate sync token for '{}'? This will generate a new token for file \
                 synchronization.",
                tenant.name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            if !config.is_json_output() {
                CliService::info("Cancelled");
            }
            let output = RotateSyncTokenOutput {
                tenant_id: tenant_id.clone(),
                status: "cancelled".to_string(),
                message: "Cancelled".to_string(),
            };
            return Ok(CommandResult::card(output).with_title("Rotate Sync Token"));
        }
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let response = if config.is_json_output() {
        client.rotate_sync_token(&tenant_id).await?
    } else {
        let spinner = CliService::spinner("Rotating sync token...");
        let resp = client.rotate_sync_token(&tenant_id).await?;
        spinner.finish_and_clear();
        resp
    };

    let tenant = store
        .tenants
        .iter_mut()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found after rotation"))?;

    tenant.sync_token = Some(response.sync_token.clone());

    store.save_to_path(&tenants_path)?;

    let output = RotateSyncTokenOutput {
        tenant_id: tenant_id.clone(),
        status: response.status.clone(),
        message: "New sync token has been saved locally.".to_string(),
    };

    if !config.is_json_output() {
        CliService::success("Sync token rotated");
        CliService::key_value("Status", &response.status);
        CliService::info("New sync token has been saved locally.");
    }

    Ok(CommandResult::card(output).with_title("Rotate Sync Token"))
}
