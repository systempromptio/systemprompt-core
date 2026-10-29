//! `cloud deploy` command: prompts, preflight, and progress rendering.
//!
//! Resolves the active cloud profile and tenant, runs the `cloud doctor`
//! preflight (deploys hard-block on a failing report), then hands the typed
//! request to [`DeployOrchestrator`] in the [`pipeline`] module, which owns
//! the image build/push, secret provisioning, and the rollout. The profile's
//! hosting provider selects the deploy provider: the managed cloud for
//! `systemprompt`, a registry push plus a `GitOps` digest commit for
//! `kubernetes` and `gke`. Rendering flows back through [`CliDeployProgress`].
//!
//! Deploys are stateless container rebuilds: runtime files created inside the
//! previous container are not preserved. Use `systemprompt cloud backup`
//! first to keep a copy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod pipeline;
pub mod progress;
pub mod select;

pub(in crate::commands::cloud) use progress::CliDeployProgress;
pub(in crate::commands::cloud) use select::resolve_profile;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::deploy::provider::{
    DeployProviderKind, GitOpsDeploy, GitOpsDeployParams, SystempromptCloud,
};
use systemprompt_cloud::{
    CloudApiClient, CloudError, CloudPath, DockerCli, ProfilePath, SystemCommandRunner,
    TenantStore, get_cloud_paths,
};
use systemprompt_identifiers::{ProfileName, TenantId};
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use pipeline::{DeployOptions, DeployOrchestrator, DeployRequest, DeploySecretsSource};

use super::tenant::get_credentials;
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use crate::shared::project::ProjectRoot;

const GITOPS_ROLLOUT_TIMEOUT: Duration = Duration::from_mins(15);

pub(super) struct DeployArgs {
    pub skip_push: bool,
    pub profile_name: Option<ProfileName>,
    pub check: bool,
    pub wait: bool,
}

async fn preflight(
    args: &DeployArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<Option<(Profile, PathBuf)>> {
    CliService::section("systemprompt.io Cloud Deploy");

    let (profile, profile_path) = resolve_profile(prompter, args.profile_name.as_ref(), config)?;

    if profile.target != systemprompt_manifest::ProfileType::Cloud {
        bail!(
            "Cannot deploy a local profile. Create a cloud profile with: systemprompt cloud \
             profile create <name>"
        );
    }

    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;
    let report = super::doctor::run(&profile, profile_dir, false).await;
    report.render();
    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    if args.check {
        CliService::success("Deploy preflight passed (--check; nothing deployed)");
        return Ok(None);
    }
    Ok(Some((profile, profile_path)))
}

pub(super) async fn execute(
    args: DeployArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    let Some((profile, profile_path)) = preflight(&args, prompter, config).await? else {
        return Ok(());
    };
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;

    let kind = DeployProviderKind::for_provider(profile.hosting_provider())?;
    let profile_name = ProfileName::try_new(profile.name.as_str()).with_context(|| {
        format!(
            "Profile name '{}' is not a valid profile name",
            profile.name
        )
    })?;
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let options = DeployOptions {
        skip_push: args.skip_push,
        wait_ready: args.wait,
    };
    let secrets = DeploySecretsSource::from_profile(
        profile.secrets.as_ref(),
        ProfilePath::Secrets.resolve(profile_dir),
    );
    let signing_key_path = super::doctor::resolve_signing_key_path(&profile, profile_dir);
    let licence_secret_required = profile.licence.enabled();
    let progress = CliDeployProgress::new();
    let orchestrator = DeployOrchestrator::new();

    match kind {
        DeployProviderKind::SystempromptCloud => {
            let target = resolve_deploy_target(&profile)?;
            let client =
                CloudApiClient::new(&target.creds.api_url, target.creds.api_token.as_str())?;
            let provider = SystempromptCloud::new(client, target.tenant_id.clone());
            let request = DeployRequest {
                tenant_id: target.tenant_id,
                tenant_name: target.tenant_name,
                profile_name,
                project_root: project.as_path().to_path_buf(),
                platform_credentials: Some(target.creds),
                secrets,
                signing_key_path,
                licence_secret_required,
                options,
            };
            orchestrator.deploy(&provider, &request, &progress).await?;
        },
        DeployProviderKind::GitOps => {
            let (tenant_id, tenant_name, provider) = gitops_provider(&profile, project.as_path())?;
            let request = DeployRequest {
                tenant_id,
                tenant_name,
                profile_name,
                project_root: project.as_path().to_path_buf(),
                platform_credentials: None,
                secrets,
                signing_key_path,
                licence_secret_required,
                options,
            };
            orchestrator.deploy(&provider, &request, &progress).await?;
        },
    }

    Ok(())
}

fn gitops_provider(
    profile: &Profile,
    project_root: &Path,
) -> Result<(TenantId, String, GitOpsDeploy)> {
    let cloud_config = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow!("No cloud configuration in profile"))?;
    let deploy = cloud_config
        .deploy
        .clone()
        .ok_or(CloudError::DeployNotConfigured {
            provider: cloud_config.provider,
        })?;
    let tenant_id = cloud_config.tenant_id.clone().ok_or_else(|| {
        anyhow!("No tenant configured. Add the tenant with 'systemprompt cloud tenant add'")
    })?;
    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("No tenant store. Add the tenant with 'systemprompt cloud tenant add'")?;
    let tenant = tenant_store.find_tenant(&tenant_id).ok_or_else(|| {
        anyhow!("Tenant {tenant_id} not found. Run 'systemprompt cloud tenant add'")
    })?;
    let hostname = tenant
        .hostname
        .clone()
        .ok_or_else(|| anyhow!("Tenant {tenant_id} has no hostname"))?;
    let provider = GitOpsDeploy::new(
        GitOpsDeployParams {
            provider: cloud_config.provider,
            tenant_name: tenant.name.clone(),
            hostname,
            deploy,
            project_root: project_root.to_path_buf(),
            rollout_timeout: GITOPS_ROLLOUT_TIMEOUT,
        },
        DockerCli::new(),
        Box::new(SystemCommandRunner),
    )?;
    Ok((tenant_id, tenant.name.clone(), provider))
}

#[derive(Debug)]
pub struct DeployTarget {
    pub tenant_id: TenantId,
    pub tenant_name: String,
    pub hostname: Option<String>,
    pub creds: systemprompt_cloud::CloudCredentials,
}

pub fn resolve_deploy_target(profile: &Profile) -> Result<DeployTarget> {
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
        bail!("Token expired. Run 'systemprompt cloud auth login' to refresh.");
    }

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud auth login'")?;

    let tenant = tenant_store.find_tenant(&tenant_id).ok_or_else(|| {
        anyhow!(
            "Tenant {} not found. Run 'systemprompt cloud auth login'",
            tenant_id
        )
    })?;

    Ok(DeployTarget {
        tenant_id,
        tenant_name: tenant.name.clone(),
        hostname: tenant.hostname.clone(),
        creds,
    })
}
