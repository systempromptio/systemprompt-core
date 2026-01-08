use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use systemprompt_cloud::constants::build;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, ProjectContext, TenantStore};
use systemprompt_core_logging::CliService;

use super::deploy_select::resolve_profile;
use super::tenant_ops::get_credentials;
use crate::common::docker::{build_docker_image, docker_login, docker_push};
use crate::common::project::ProjectRoot;

#[derive(Debug)]
pub struct DeployConfig {
    pub binary: PathBuf,
    pub web_dist: PathBuf,
    pub web_images: PathBuf,
    pub dockerfile: PathBuf,
}

impl DeployConfig {
    pub fn from_project(project: &ProjectRoot, profile_name: &str) -> Result<Self> {
        let root = project.as_path();
        let binary = root
            .join(build::CARGO_TARGET)
            .join("release")
            .join(build::BINARY_NAME);
        let web_dist = root.join(build::WEB_DIST);
        let web_images = root.join(build::WEB_IMAGES);

        let ctx = ProjectContext::new(root.to_path_buf());
        let dockerfile = ctx.profile_dockerfile(profile_name);

        let config = Self {
            binary,
            web_dist,
            web_images,
            dockerfile,
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

        if !self.web_dist.exists() {
            return Err(anyhow!(
                "Web dist not found: {}\n\nRun: npm run build",
                self.web_dist.display()
            ));
        }

        let index_html = self.web_dist.join("index.html");
        if !index_html.exists() {
            return Err(anyhow!(
                "Web dist missing index.html: {}\n\nRun: npm run build",
                self.web_dist.display()
            ));
        }

        if !self.web_images.exists() {
            return Err(anyhow!(
                "Web images directory not found: {}\n\nEnsure core/web/src/assets/images/ exists \
                 with blog/, social/, and logos/ subdirectories",
                self.web_images.display()
            ));
        }

        self.validate_images_structure()?;

        if !self.dockerfile.exists() {
            return Err(anyhow!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            ));
        }

        Ok(())
    }

    fn validate_images_structure(&self) -> Result<()> {
        let required_subdirs = ["blog", "social", "logos"];
        let mut missing = Vec::new();

        for subdir in required_subdirs {
            let path = self.web_images.join(subdir);
            if !path.exists() {
                missing.push(subdir);
            }
        }

        if !missing.is_empty() {
            return Err(anyhow!(
                "Web images missing required subdirectories: {}\n\nRequired structure:\n  \
                 {}/\n    blog/\n    social/\n    logos/",
                missing.join(", "),
                self.web_images.display()
            ));
        }

        Ok(())
    }
}

pub async fn execute(skip_push: bool, profile_name: Option<String>) -> Result<()> {
    CliService::section("SystemPrompt Cloud Deploy");

    let (profile, profile_path) = resolve_profile(profile_name.as_deref())?;

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
    CliService::key_value("Web dist", &config.web_dist.display().to_string());
    CliService::key_value("Web images", &config.web_images.display().to_string());
    CliService::key_value("Dockerfile", &config.dockerfile.display().to_string());

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
    let secrets_path = profile_dir.join("secrets.json");

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

    let profile_env_path = format!("/app/services/profiles/{}/profile.yaml", profile.name);
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
    let secrets_path = profile_dir.join("secrets.json");

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

    let profile_env_path = format!("/app/services/profiles/{}/profile.yaml", profile_name);
    let spinner = CliService::spinner("Setting profile path...");
    let mut profile_secret = std::collections::HashMap::new();
    profile_secret.insert("SYSTEMPROMPT_PROFILE".to_string(), profile_env_path);
    client.set_secrets(tenant_id, profile_secret).await?;
    spinner.finish_and_clear();
    CliService::success("Profile path configured");

    Ok(())
}
