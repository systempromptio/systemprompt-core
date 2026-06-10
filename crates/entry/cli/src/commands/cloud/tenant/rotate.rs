use anyhow::{Result, anyhow, bail};
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use systemprompt_cloud::{CloudApiClient, CloudPath, TenantStore, TenantType, get_cloud_paths};
use systemprompt_logging::CliService;

use super::select::{get_credentials, select_tenant};
use crate::cli_settings::CliConfig;
use crate::cloud::types::RotateCredentialsOutput;
use crate::shared::CommandOutput;

pub async fn rotate_credentials(
    id: Option<String>,
    skip_confirm: bool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let mut store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        if !config.is_json_output() {
            CliService::warning(&format!("Failed to load tenant store: {}", e));
        }
        TenantStore::default()
    });

    let tenant_id = resolve_rotation_target(id, &store, skip_confirm)?;

    let tenant = store
        .tenants
        .iter()
        .find(|t| t.id == tenant_id)
        .ok_or_else(|| anyhow!("Tenant not found: {}", tenant_id))?;

    if tenant.tenant_type != TenantType::Cloud {
        bail!("Credential rotation is only available for cloud tenants");
    }

    if !skip_confirm && !confirm_rotation(&tenant.name)? {
        return Ok(cancelled_output(&tenant_id, config));
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let response = if config.is_json_output() {
        client
            .rotate_credentials(&systemprompt_identifiers::TenantId::new(&tenant_id))
            .await?
    } else {
        let spinner = CliService::spinner("Rotating database credentials...");
        let resp = client
            .rotate_credentials(&systemprompt_identifiers::TenantId::new(&tenant_id))
            .await?;
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
        tenant: tenant_id.clone(),
        status: response.status.clone(),
        internal_database_url: response.internal_database_url.clone(),
        external_database_url: response.external_database_url,
    };

    if !config.is_json_output() {
        render_rotation_result(&output);
    }

    Ok(CommandOutput::card_value("Rotate Credentials", &output))
}

fn resolve_rotation_target(
    id: Option<String>,
    store: &TenantStore,
    skip_confirm: bool,
) -> Result<String> {
    if let Some(id) = id {
        return Ok(id);
    }
    if store.tenants.is_empty() {
        bail!("No tenants configured.");
    }
    if skip_confirm {
        bail!("Tenant ID required in non-interactive mode");
    }
    Ok(select_tenant(&store.tenants)?.id.clone())
}

fn confirm_rotation(tenant_name: &str) -> Result<bool> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Rotate database credentials for '{}'? This will generate a new password.",
            tenant_name
        ))
        .default(false)
        .interact()?)
}

fn cancelled_output(tenant_id: &str, config: &CliConfig) -> CommandOutput {
    if !config.is_json_output() {
        CliService::info("Cancelled");
    }
    let output = RotateCredentialsOutput {
        tenant: tenant_id.to_owned(),
        status: "cancelled".to_owned(),
        internal_database_url: String::new(),
        external_database_url: String::new(),
    };
    CommandOutput::card_value("Rotate Credentials", &output)
}

fn render_rotation_result(output: &RotateCredentialsOutput) {
    CliService::success("Database credentials rotated");
    CliService::key_value("Status", &output.status);

    CliService::section("New Database Connection");
    CliService::key_value("Internal URL", &output.internal_database_url);
    CliService::key_value("External URL", &output.external_database_url);
}
