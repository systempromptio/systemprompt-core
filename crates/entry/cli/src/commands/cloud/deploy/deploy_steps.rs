use std::collections::HashMap;

use anyhow::{Result, anyhow};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::{CloudApiClient, ProfilePath, ProjectContext};
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;

use super::super::dockerfile::validate_profile_dockerfile;
use super::super::secrets::sync_cloud_credentials;
use super::super::tenant::{find_services_config, get_credentials};
use crate::shared::docker::{build_docker_image, docker_login, docker_push};
use crate::shared::project::ProjectRoot;

pub async fn deploy_with_secrets(
    client: &CloudApiClient,
    tenant_id: &str,
    profile_name: &str,
) -> Result<()> {
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let ctx = ProjectContext::new(project.as_path().to_path_buf());
    let dockerfile = ctx.profile_dockerfile(profile_name);
    let services_config_path = find_services_config(project.as_path())?;
    let services_config = ConfigLoader::load_from_path(&services_config_path)?;

    validate_profile_dockerfile(&dockerfile, project.as_path(), &services_config)?;

    let spinner = CliService::spinner("Fetching registry credentials...");
    let registry_token = client.get_registry_token(tenant_id).await?;
    spinner.finish_and_clear();

    let image = format!(
        "{}/{}:{}",
        registry_token.registry, registry_token.repository, registry_token.tag
    );
    CliService::key_value("Image", &image);

    let spinner = CliService::spinner("Building Docker image...");
    build_docker_image(project.as_path(), &dockerfile, &image)?;
    spinner.finish_and_clear();
    CliService::success("Docker image built");

    let spinner = CliService::spinner("Pushing to registry...");
    docker_login(
        &registry_token.registry,
        &registry_token.username,
        &registry_token.token,
    )?;
    docker_push(&image)?;
    spinner.finish_and_clear();
    CliService::success("Image pushed");

    let spinner = CliService::spinner("Deploying...");
    let response = client.deploy(tenant_id, &image).await?;
    spinner.finish_and_clear();
    CliService::success("Deployed!");
    if let Some(url) = response.app_url {
        CliService::key_value("URL", &url);
    }

    let ctx = ProjectContext::discover();
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
    let mut profile_secret = HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_string(), profile_env_path);
    client.set_secrets(tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}
