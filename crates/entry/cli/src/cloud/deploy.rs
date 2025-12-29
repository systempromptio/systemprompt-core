//! Cloud deploy command - builds and deploys to SystemPrompt Cloud

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudError, CloudPath, CredentialsBootstrap, TenantStore,
};
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::Profile;

use crate::common::docker::{build_docker_image, docker_login, docker_push};
use crate::common::project::ProjectRoot;

#[derive(Debug)]
pub struct DeployConfig {
    pub binary: PathBuf,
    pub web_dist: PathBuf,
    pub dockerfile: PathBuf,
}

impl DeployConfig {
    pub fn from_profile(profile: &Profile) -> Result<Self> {
        let paths = &profile.paths;

        let cargo_target = paths
            .cargo_target
            .as_ref()
            .ok_or_else(|| CloudError::missing_cargo_target())?;
        let binary = PathBuf::from(cargo_target).join("release/systemprompt");

        let web_dist = paths
            .web_dist
            .as_ref()
            .ok_or_else(|| CloudError::missing_web_dist())?;
        let web_dist = PathBuf::from(web_dist);

        let dockerfile = paths
            .dockerfile
            .as_ref()
            .ok_or_else(|| CloudError::missing_dockerfile())?;
        let dockerfile = PathBuf::from(dockerfile);

        let config = Self {
            binary,
            web_dist,
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

        if !self.dockerfile.exists() {
            return Err(anyhow!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            ));
        }

        Ok(())
    }
}

pub async fn execute(skip_push: bool, custom_tag: Option<String>) -> Result<()> {
    CliService::section("SystemPrompt Cloud Deploy");

    let creds = CredentialsBootstrap::require()
        .context("Deployment requires cloud credentials. Run 'systemprompt cloud login'")?;

    let profile = ProfileBootstrap::get()
        .context("Profile required for deployment. Set SYSTEMPROMPT_PROFILE")?;

    if let Some(cloud) = &profile.cloud {
        if !cloud.enabled {
            bail!("Cloud features are disabled in this profile. Set cloud.enabled: true");
        }
    }

    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|c| c.tenant_id.as_ref())
        .ok_or_else(|| anyhow!("No tenant configured. Run 'systemprompt cloud config'"))?;

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

    let app_id = tenant
        .app_id
        .as_ref()
        .ok_or_else(|| anyhow!("No app configured for tenant. Run 'systemprompt cloud setup'"))?;

    let tenant_name = &tenant.name;

    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;

    let config = DeployConfig::from_profile(profile)?;

    CliService::key_value("Tenant", tenant_name);
    CliService::key_value("Binary", &config.binary.display().to_string());
    CliService::key_value("Web dist", &config.web_dist.display().to_string());
    CliService::key_value("Dockerfile", &config.dockerfile.display().to_string());

    let tag = custom_tag.unwrap_or_else(|| {
        let timestamp = chrono::Utc::now().timestamp();
        let git_sha = get_git_sha().unwrap_or_else(|| "unknown".to_string());
        format!("deploy-{timestamp}-{git_sha}")
    });
    let image = format!("registry.fly.io/{app_id}:{tag}");
    CliService::key_value("Image", &image);

    let spinner = CliService::spinner("Building Docker image...");
    build_docker_image(project.as_path(), &config.dockerfile, &image)?;
    spinner.finish_and_clear();
    CliService::success("Docker image built");

    let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    if !skip_push {
        let spinner = CliService::spinner("Pushing to registry...");
        let registry_token = api_client.get_registry_token(tenant_id).await?;
        docker_login(
            &registry_token.registry,
            &registry_token.username,
            &registry_token.password,
        )?;
        docker_push(&image)?;
        spinner.finish_and_clear();
        CliService::success("Image pushed");
    } else {
        CliService::info("Push skipped (--skip-push)");
    }

    let spinner = CliService::spinner("Deploying...");
    let response = api_client.deploy(tenant_id, &image).await?;
    spinner.finish_and_clear();
    CliService::success("Deployed!");
    CliService::key_value("Status", &response.status);
    if let Some(url) = response.app_url {
        CliService::key_value("URL", &url);
    }

    Ok(())
}

fn get_git_sha() -> Option<String> {
    match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => String::from_utf8(output.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|e| tracing::debug!(error = %e, "Git SHA not valid UTF-8"))
            .ok(),
        Ok(output) => {
            tracing::debug!(status = ?output.status, "Git command failed");
            None
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to execute git command");
            None
        },
    }
}
