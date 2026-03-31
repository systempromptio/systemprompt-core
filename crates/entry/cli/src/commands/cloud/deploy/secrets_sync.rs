use std::path::Path;

use anyhow::{Result, anyhow};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::{CloudApiClient, ProfilePath};
use systemprompt_logging::CliService;

use super::super::secrets::sync_cloud_credentials;
use super::super::tenant::get_credentials;

pub async fn sync_secrets_after_deploy(
    api_client: &CloudApiClient,
    tenant_id: &str,
    profile_name: &str,
    profile_path: &Path,
) -> Result<()> {
    CliService::section("Syncing Secrets");
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if secrets_path.exists() {
        let secrets = super::super::secrets::load_secrets_json(&secrets_path)?;
        if !secrets.is_empty() {
            let env_secrets = super::super::secrets::map_secrets_to_env_vars(secrets);
            let spinner = CliService::spinner("Syncing secrets...");
            let keys = api_client.set_secrets(tenant_id, env_secrets).await?;
            spinner.finish_and_clear();
            CliService::success(&format!("Synced {} secrets", keys.len()));
        }
    } else {
        CliService::warning("No secrets.json found - skipping secrets sync");
    }

    CliService::section("Syncing Cloud Credentials");
    let creds = get_credentials()?;
    let spinner = CliService::spinner("Syncing cloud credentials...");
    let keys = sync_cloud_credentials(api_client, tenant_id, &creds).await?;
    spinner.finish_and_clear();
    CliService::success(&format!("Synced {} cloud credentials", keys.len()));

    let profile_env_path = format!(
        "{}/{}/{}",
        container::PROFILES,
        profile_name,
        paths::PROFILE_CONFIG
    );
    let spinner = CliService::spinner("Setting profile path...");
    let mut profile_secret = std::collections::HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_string(), profile_env_path);
    api_client.set_secrets(tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}

pub async fn sync_secrets_for_deploy(
    client: &CloudApiClient,
    tenant_id: &str,
    profile_name: &str,
) -> Result<()> {
    let ctx = systemprompt_cloud::ProjectContext::discover();
    let profile_dir = ctx.profile_dir(profile_name);
    let secrets_path = ProfilePath::Secrets.resolve(&profile_dir);

    if secrets_path.exists() {
        let secrets = super::super::secrets::load_secrets_json(&secrets_path)?;
        if !secrets.is_empty() {
            let env_secrets = super::super::secrets::map_secrets_to_env_vars(secrets);
            let spinner = CliService::spinner("Syncing secrets...");
            let keys = client.set_secrets(tenant_id, env_secrets).await?;
            spinner.finish_and_clear();
            CliService::success(&format!("Synced {} secrets", keys.len()));
        }
    }

    let creds = get_credentials()?;
    let spinner = CliService::spinner("Syncing cloud credentials...");
    let keys = sync_cloud_credentials(client, tenant_id, &creds).await?;
    spinner.finish_and_clear();
    CliService::success(&format!("Synced {} cloud credentials", keys.len()));

    let profile_env_path = format!(
        "{}/{}/{}",
        container::PROFILES,
        profile_name,
        paths::PROFILE_CONFIG
    );
    let spinner = CliService::spinner("Setting profile path...");
    let mut profile_secret = std::collections::HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_string(), profile_env_path);
    client.set_secrets(tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}
