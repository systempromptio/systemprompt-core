//! `cloud deploy` orchestration: build, push, and release a tenant image.
//!
//! Resolves the active cloud profile and tenant, runs the pre-deploy sync,
//! builds and pushes the Docker image to the tenant registry, triggers the
//! deploy, and syncs secrets, cloud credentials, and the profile path.

mod config;
mod deploy_steps;
mod pre_sync;
mod pre_sync_config;
mod pre_sync_display;
mod select;

pub(super) use deploy_steps::deploy_with_secrets;
pub(in crate::commands::cloud) use select::resolve_profile;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::{CloudApiClient, CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;

pub(super) use super::secrets::sync_cloud_credentials;
use super::tenant::get_credentials;
use crate::cli_settings::CliConfig;
use crate::shared::project::ProjectRoot;
use config::DeployConfig;
use systemprompt_cloud::deploy::{find_services_config, validate_profile_dockerfile};
use systemprompt_cloud::{DockerCli, secrets_env};
use systemprompt_loader::ConfigLoader;

pub(super) struct DeployArgs {
    pub skip_push: bool,
    pub profile_name: Option<String>,
    pub no_sync: bool,
    pub yes: bool,
    pub dry_run: bool,
    pub check: bool,
}

pub(super) async fn execute(args: DeployArgs, config: &CliConfig) -> Result<()> {
    CliService::section("systemprompt.io Cloud Deploy");

    let (profile, profile_path) = resolve_profile(args.profile_name.as_deref(), config)?;

    if profile.target != systemprompt_models::ProfileType::Cloud {
        bail!(
            "Cannot deploy a local profile. Create a cloud profile with: systemprompt cloud \
             profile create <name>"
        );
    }

    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;
    let report = super::doctor::run(&profile, profile_dir).await;
    report.render();
    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    if args.check {
        CliService::success("Deploy preflight passed (--check; nothing deployed)");
        return Ok(());
    }

    let target = resolve_deploy_target(&profile)?;

    let sync_result = pre_sync::execute(
        &target.tenant_id,
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

    let (project, deploy_config) = prepare_build(&profile.name, &target.tenant_name)?;

    let api_client = CloudApiClient::new(&target.creds.api_url, &target.creds.api_token)?;
    let image = build_and_push_image(
        &api_client,
        &target.tenant_id,
        &project,
        &deploy_config,
        args.skip_push,
    )
    .await?;

    provision_secrets(
        &api_client,
        &target.tenant_id,
        &profile,
        profile_dir,
        &target.creds,
    )
    .await?;

    let spinner = CliService::spinner("Deploying...");
    let response = api_client.deploy(&target.tenant_id, &image).await?;
    spinner.finish_and_clear();
    CliService::success("Deployed!");
    CliService::key_value("Status", &response.status);
    if let Some(url) = response.app_url {
        CliService::key_value("URL", &url);
    }

    Ok(())
}

struct DeployTarget {
    tenant_id: TenantId,
    tenant_name: String,
    creds: systemprompt_cloud::CloudCredentials,
}

fn resolve_deploy_target(profile: &systemprompt_models::Profile) -> Result<DeployTarget> {
    let cloud_config = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow!("No cloud configuration in profile"))?;

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

    Ok(DeployTarget {
        tenant_id,
        tenant_name: tenant.name.clone(),
        creds,
    })
}

fn prepare_build(profile_name: &str, tenant_name: &str) -> Result<(ProjectRoot, DeployConfig)> {
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;

    let deploy_config = DeployConfig::from_project(&project, profile_name)?;

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

    Ok((project, deploy_config))
}

async fn build_and_push_image(
    api_client: &CloudApiClient,
    tenant_id: &TenantId,
    project: &ProjectRoot,
    deploy_config: &DeployConfig,
    skip_push: bool,
) -> Result<String> {
    let spinner = CliService::spinner("Fetching registry credentials...");
    let registry_token = api_client.get_registry_token(tenant_id).await?;
    spinner.finish_and_clear();

    let image = format!(
        "{}/{}:{}",
        registry_token.registry, registry_token.repository, registry_token.tag
    );
    CliService::key_value("Image", &image);

    let docker = DockerCli::new();
    let spinner = CliService::spinner("Building Docker image...");
    docker.build_image(project.as_path(), &deploy_config.dockerfile, &image)?;
    spinner.finish_and_clear();
    CliService::success("Docker image built");

    if skip_push {
        CliService::info("Push skipped (--skip-push)");
    } else {
        let spinner = CliService::spinner("Pushing to registry...");
        docker.login(
            &registry_token.registry,
            &registry_token.username,
            &registry_token.token,
        )?;
        docker.push(&image)?;
        spinner.finish_and_clear();
        CliService::success("Image pushed");
    }

    Ok(image)
}

async fn provision_secrets(
    api_client: &CloudApiClient,
    tenant_id: &TenantId,
    profile: &systemprompt_models::Profile,
    profile_dir: &std::path::Path,
    creds: &systemprompt_cloud::CloudCredentials,
) -> Result<()> {
    CliService::section("Provisioning Secrets");

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let mut env_secrets = if secrets_path.exists() {
        secrets_env::map_secrets_to_env_vars(secrets_env::load_secrets_json(&secrets_path)?)
    } else {
        CliService::warning("No secrets.json found - skipping secrets sync");
        std::collections::HashMap::new()
    };

    if !env_secrets.contains_key("SIGNING_KEY_PEM")
        && let Some(pem) = read_signing_key_pem(profile, profile_dir)?
    {
        env_secrets.insert("SIGNING_KEY_PEM".to_owned(), pem);
    }

    if !env_secrets.is_empty() {
        let spinner = CliService::spinner("Syncing secrets...");
        let keys = api_client.set_secrets(tenant_id, env_secrets).await?;
        spinner.finish_and_clear();
        CliService::success(&format!("Synced {} secrets", keys.len()));
    }

    let spinner = CliService::spinner("Syncing cloud credentials...");
    let keys = sync_cloud_credentials(api_client, tenant_id, creds).await?;
    spinner.finish_and_clear();
    CliService::success(&format!("Synced {} cloud credentials", keys.len()));

    let profile_env_path = format!(
        "{}/{}/{}",
        container::PROFILES,
        profile.name,
        paths::PROFILE_CONFIG
    );
    let mut profile_secret = std::collections::HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_owned(), profile_env_path);
    api_client.set_secrets(tenant_id, profile_secret).await?;
    CliService::success("Profile path configured");

    Ok(())
}

fn read_signing_key_pem(
    profile: &systemprompt_models::Profile,
    profile_dir: &std::path::Path,
) -> Result<Option<String>> {
    let path = super::doctor::resolve_signing_key_path(profile, profile_dir);
    secrets_env::read_signing_key_pem(&path).map_err(Into::into)
}
