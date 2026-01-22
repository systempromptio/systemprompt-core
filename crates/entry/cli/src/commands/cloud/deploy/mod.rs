mod select;

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use systemprompt_cloud::constants::{build, container, paths};
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudPath, ProfilePath, ProjectContext, TenantStore,
};
use systemprompt_logging::CliService;

use super::dockerfile::validate_profile_dockerfile;
use super::secrets::sync_cloud_credentials;
use super::tenant::{find_services_config, get_credentials};
use crate::cli_settings::CliConfig;
use crate::shared::docker::{build_docker_image, docker_login, docker_push};
use crate::shared::project::ProjectRoot;
use select::resolve_profile;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::ConfigLoader;

#[derive(Debug)]
pub struct DeployConfig {
    pub binary: PathBuf,
    pub dockerfile: PathBuf,
    project_root: PathBuf,
}

impl DeployConfig {
    pub fn from_project(project: &ProjectRoot, profile_name: &str) -> Result<Self> {
        let root = project.as_path();
        let binary = root
            .join(build::CARGO_TARGET)
            .join("release")
            .join(build::BINARY_NAME);

        let ctx = ProjectContext::new(root.to_path_buf());
        let dockerfile = ctx.profile_dockerfile(profile_name);

        let config = Self {
            binary,
            dockerfile,
            project_root: root.to_path_buf(),
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.binary.exists() {
            return Err(anyhow!(
                "Release binary not found: {}\n\nRun: cargo build --release --bin systemprompt",
                self.binary.display()
            ));
        }

        self.validate_extension_assets()?;
        self.validate_storage_directory()?;
        self.validate_templates_directory()?;

        if !self.dockerfile.exists() {
            return Err(anyhow!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            ));
        }

        Ok(())
    }

    fn validate_extension_assets(&self) -> Result<()> {
        let registry = ExtensionRegistry::discover();
        let mut missing = Vec::new();
        let mut outside_context = Vec::new();

        for ext in registry.asset_extensions() {
            let ext_id = ext.id();
            for asset in ext.required_assets() {
                if !asset.is_required() {
                    continue;
                }

                let source = asset.source();

                if !source.exists() {
                    missing.push(format!("[ext:{}] {}", ext_id, source.display()));
                    continue;
                }

                if !source.starts_with(&self.project_root) {
                    outside_context.push(format!(
                        "[ext:{}] {} (not under {})",
                        ext_id,
                        source.display(),
                        self.project_root.display()
                    ));
                }
            }
        }

        if !missing.is_empty() {
            bail!(
                "Missing required extension assets:\n  {}\n\nCreate these files or mark them as \
                 optional.",
                missing.join("\n  ")
            );
        }

        if !outside_context.is_empty() {
            bail!(
                "Extension assets outside Docker build context:\n  {}\n\nMove assets inside the \
                 project directory.",
                outside_context.join("\n  ")
            );
        }

        Ok(())
    }

    fn validate_storage_directory(&self) -> Result<()> {
        let storage_dir = self.project_root.join("storage");

        if !storage_dir.exists() {
            bail!(
                "Storage directory not found: {}\n\nExpected: storage/\n\nCreate this directory \
                 for files, images, and other assets.",
                storage_dir.display()
            );
        }

        let files_dir = storage_dir.join("files");
        if !files_dir.exists() {
            bail!(
                "Storage files directory not found: {}\n\nExpected: storage/files/\n\nThis \
                 directory is required for serving static assets.",
                files_dir.display()
            );
        }

        Ok(())
    }

    fn validate_templates_directory(&self) -> Result<()> {
        let templates_dir = self.project_root.join("services/web/templates");

        if !templates_dir.exists() {
            bail!(
                "Templates directory not found: {}\n\nExpected: services/web/templates/\n\nCreate \
                 this directory with your HTML templates.",
                templates_dir.display()
            );
        }

        Ok(())
    }
}


pub async fn execute(
    skip_push: bool,
    profile_name: Option<String>,
    config: &CliConfig,
) -> Result<()> {
    CliService::section("systemprompt.io Cloud Deploy");

    let (profile, profile_path) = resolve_profile(profile_name.as_deref(), config)?;

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
        .ok_or_else(|| anyhow!("No tenant configured. Run 'systemprompt cloud config'"))?;

    let creds = get_credentials()?;
    if creds.is_token_expired() {
        bail!("Token expired. Run 'systemprompt cloud login' to refresh.");
    }

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store.find_tenant(tenant_id).ok_or_else(|| {
        anyhow!(
            "Tenant {} not found. Run 'systemprompt cloud login'",
            tenant_id
        )
    })?;

    let tenant_name = &tenant.name;

    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;

    let config = DeployConfig::from_project(&project, &profile.name)?;

    CliService::key_value("Tenant", tenant_name);
    CliService::key_value("Binary", &config.binary.display().to_string());
    CliService::key_value("Dockerfile", &config.dockerfile.display().to_string());

    let services_config_path = find_services_config(project.as_path())?;
    let services_config = ConfigLoader::load_from_path(&services_config_path)?;
    validate_profile_dockerfile(&config.dockerfile, project.as_path(), &services_config)?;

    let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner("Fetching registry credentials...");
    let registry_token = api_client.get_registry_token(tenant_id).await?;
    spinner.finish_and_clear();

    let image = format!(
        "{}/{}:{}",
        registry_token.registry, registry_token.repository, registry_token.tag
    );
    CliService::key_value("Image", &image);

    let spinner = CliService::spinner("Building Docker image...");
    build_docker_image(project.as_path(), &config.dockerfile, &image)?;
    spinner.finish_and_clear();
    CliService::success("Docker image built");

    if skip_push {
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
    let response = api_client.deploy(tenant_id, &image).await?;
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
            let keys = api_client.set_secrets(tenant_id, env_secrets).await?;
            spinner.finish_and_clear();
            CliService::success(&format!("Synced {} secrets", keys.len()));
        }
    } else {
        CliService::warning("No secrets.json found - skipping secrets sync");
    }

    CliService::section("Syncing Cloud Credentials");
    let spinner = CliService::spinner("Syncing cloud credentials...");
    let keys = sync_cloud_credentials(&api_client, tenant_id, &creds).await?;
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
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_string(), profile_env_path);
    api_client.set_secrets(tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}

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
        let secrets = super::secrets::load_secrets_json(&secrets_path)?;
        if !secrets.is_empty() {
            let env_secrets = super::secrets::map_secrets_to_env_vars(secrets);
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
