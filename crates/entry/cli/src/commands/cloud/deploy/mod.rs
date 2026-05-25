mod config;
mod deploy_steps;
mod pre_sync;
mod pre_sync_config;
mod pre_sync_display;
mod select;

pub(crate) use deploy_steps::deploy_with_secrets;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::{CloudApiClient, CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;

use super::dockerfile::validate_profile_dockerfile;
pub(crate) use super::secrets::sync_cloud_credentials;
use super::tenant::{find_services_config, get_credentials};
use crate::cli_settings::CliConfig;
use crate::shared::docker::{build_docker_image, docker_login, docker_push};
use crate::shared::project::ProjectRoot;
use config::DeployConfig;
use select::resolve_profile;
use systemprompt_loader::ConfigLoader;

pub(crate) struct DeployArgs {
    pub skip_push: bool,
    pub profile_name: Option<String>,
    pub no_sync: bool,
    pub yes: bool,
    pub dry_run: bool,
}

pub(crate) async fn execute(args: DeployArgs, config: &CliConfig) -> Result<()> {
    CliService::section("systemprompt.io Cloud Deploy");

    let (profile, profile_path) = resolve_profile(args.profile_name.as_deref(), config)?;

    let cloud_config = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow!("No cloud configuration in profile"))?;

    if profile.target != systemprompt_models::ProfileType::Cloud {
        bail!(
            "Cannot deploy a local profile. Create a cloud profile with: systemprompt cloud \
             profile create <name>"
        );
    }

    let tenant_id = cloud_config
        .tenant_id
        .as_ref()
        .map(TenantId::new)
        .ok_or_else(|| anyhow!("No tenant configured. Run 'systemprompt cloud config'"))?;

    let creds = get_credentials()?;
    if creds.is_token_expired() {
        bail!("Token expired. Run 'systemprompt cloud login' to refresh.");
    }

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store
        .find_tenant(tenant_id.as_str())
        .ok_or_else(|| {
            anyhow!(
                "Tenant {} not found. Run 'systemprompt cloud login'",
                tenant_id
            )
        })?;

    let tenant_name = &tenant.name;

    let sync_result = pre_sync::execute(
        &tenant_id,
        pre_sync::PreSyncConfig {
            no_sync: args.no_sync,
            yes: args.yes,
            dry_run: args.dry_run,
        },
        config,
        &profile_path,
    )
    .await?;

    if sync_result.dry_run {
        CliService::info("Dry run complete. No deployment performed.");
        return Ok(());
    }

    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;

    let deploy_config = DeployConfig::from_project(&project, &profile.name)?;

    CliService::key_value("Tenant", tenant_name);
    CliService::key_value("Binary", &deploy_config.binary.display().to_string());
    CliService::key_value(
        "Dockerfile",
        &deploy_config.dockerfile.display().to_string(),
    );

    let services_config_path = find_services_config(project.as_path())?;
    let services_config = ConfigLoader::load_from_path(&services_config_path)?;
    validate_profile_dockerfile(
        &deploy_config.dockerfile,
        project.as_path(),
        &services_config,
    )?;

    let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let spinner = CliService::spinner("Fetching registry credentials...");
    let registry_token = api_client.get_registry_token(&tenant_id).await?;
    spinner.finish_and_clear();

    let image = format!(
        "{}/{}:{}",
        registry_token.registry, registry_token.repository, registry_token.tag
    );
    CliService::key_value("Image", &image);

    let spinner = CliService::spinner("Building Docker image...");
    build_docker_image(project.as_path(), &deploy_config.dockerfile, &image)?;
    spinner.finish_and_clear();
    CliService::success("Docker image built");

    if args.skip_push {
        CliService::info("Push skipped (--skip-push)");
    } else {
        let spinner = CliService::spinner("Pushing to registry...");
        docker_login(
            &registry_token.registry,
            &registry_token.username,
            &registry_token.token,
        )?;
        docker_push(&image)?;
        spinner.finish_and_clear();
        CliService::success("Image pushed");
    }

    let spinner = CliService::spinner("Deploying...");
    let response = api_client.deploy(&tenant_id, &image).await?;
    spinner.finish_and_clear();
    CliService::success("Deployed!");
    CliService::key_value("Status", &response.status);
    if let Some(url) = response.app_url {
        CliService::key_value("URL", &url);
    }

    CliService::section("Syncing Secrets");
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if secrets_path.exists() {
        let secrets = super::secrets::load_secrets_json(&secrets_path)?;
        if !secrets.is_empty() {
            let env_secrets = super::secrets::map_secrets_to_env_vars(secrets);
            let spinner = CliService::spinner("Syncing secrets...");
            let keys = api_client.set_secrets(&tenant_id, env_secrets).await?;
            spinner.finish_and_clear();
            CliService::success(&format!("Synced {} secrets", keys.len()));
        }
    } else {
        CliService::warning("No secrets.json found - skipping secrets sync");
    }

    CliService::section("Syncing Cloud Credentials");
    let spinner = CliService::spinner("Syncing cloud credentials...");
    let keys = sync_cloud_credentials(&api_client, &tenant_id, &creds).await?;
    spinner.finish_and_clear();
    CliService::success(&format!("Synced {} cloud credentials", keys.len()));

    let profile_env_path = format!(
        "{}/{}/{}",
        container::PROFILES,
        profile.name,
        paths::PROFILE_CONFIG
    );
    let spinner = CliService::spinner("Setting profile path...");
    let mut profile_secret = std::collections::HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_owned(), profile_env_path);
    api_client.set_secrets(&tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}
