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
pub mod target_profile;

pub(in crate::commands::cloud) use progress::CliDeployProgress;
pub(in crate::commands::cloud) use select::resolve_profile;
pub use target_profile::DeployProfile;

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::deploy::provider::{
    DeployProviderKind, GitOpsDeploy, GitOpsDeployParams, ReadinessWait, SystempromptCloud,
};
use systemprompt_cloud::{
    CloudApiClient, CloudError, CloudPath, DockerCli, SystemCommandRunner, TenantStore,
    get_cloud_paths,
};
use systemprompt_identifiers::{ProfileName, TenantId};
use systemprompt_logging::CliService;
use systemprompt_manifest::profile::CloudConfig;

use pipeline::{DeployOptions, DeployOrchestrator, DeployRequest};

use super::tenant::get_credentials;
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use crate::shared::project::ProjectRoot;

const GITOPS_READINESS_WAIT: ReadinessWait = ReadinessWait {
    timeout: Duration::from_mins(15),
    interval: Duration::from_secs(5),
};

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
) -> Result<Option<DeployProfile>> {
    CliService::section("systemprompt.io Cloud Deploy");

    let profile = resolve_profile(prompter, args.profile_name.as_ref(), config)?;

    if profile.target != systemprompt_manifest::ProfileType::Cloud {
        bail!(
            "Cannot deploy a local profile. Create a cloud profile with: systemprompt cloud \
             profile create <name>"
        );
    }

    let report = super::doctor::run_managed(&profile.managed, profile.profile_dir(), false).await;
    report.render();
    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    if !args.check && report.not_evaluated() > 0 {
        report.render_summary("Deploy preflight passed");
    }
    if args.check {
        report.render_summary("Deploy preflight passed (--check; nothing deployed)");
        return Ok(None);
    }
    Ok(Some(profile))
}

pub(super) async fn execute(
    args: DeployArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    let Some(profile) = preflight(&args, prompter, config).await? else {
        return Ok(());
    };

    let kind = DeployProviderKind::for_provider(profile.hosting_provider())?;
    let profile_name = profile.name.clone();
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let options = DeployOptions {
        skip_push: args.skip_push,
        wait_ready: args.wait,
    };
    let secrets = profile.secrets.clone();
    let signing_key_path = profile.signing_key_path.clone();
    let licence_secret_required = profile.licence_secret_required;
    let progress = CliDeployProgress::new();
    let orchestrator = DeployOrchestrator::new();

    match kind {
        DeployProviderKind::SystempromptCloud => {
            let target = resolve_deploy_target(profile.cloud.as_ref())?;
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
            let orchestrator = orchestrator.with_readiness(GITOPS_READINESS_WAIT);
            let (tenant_id, tenant_name, provider) =
                gitops_provider(profile.cloud.as_ref(), project.as_path())?;
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
    cloud: Option<&CloudConfig>,
    project_root: &Path,
) -> Result<(TenantId, String, GitOpsDeploy)> {
    let cloud_config = cloud.ok_or_else(|| anyhow!("No cloud configuration in profile"))?;
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

pub fn resolve_deploy_target(cloud: Option<&CloudConfig>) -> Result<DeployTarget> {
    let cloud_config = cloud.ok_or_else(|| anyhow!("No cloud configuration in profile"))?;

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
