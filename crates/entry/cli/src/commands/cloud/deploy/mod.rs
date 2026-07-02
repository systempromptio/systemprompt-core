//! `cloud deploy` command: prompts, preflight, and progress rendering.
//!
//! Resolves the active cloud profile and tenant, runs the `cloud doctor`
//! preflight (deploys hard-block on a failing report), then hands the typed
//! request to [`DeployOrchestrator`] in `systemprompt-sync`, which owns the
//! pre-deploy sync, image build/push, secret provisioning, and the deploy
//! call. Rendering flows back through [`CliDeployProgress`].

pub mod progress;
mod select;

pub(in crate::commands::cloud) use progress::CliDeployProgress;
pub(in crate::commands::cloud) use select::resolve_profile;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::{CloudPath, ProfilePath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_sync::deploy::{
    DeployOptions, DeployOrchestrator, DeployOutcome, DeployRequest, PreSyncOptions,
};

use super::tenant::get_credentials;
use crate::cli_settings::CliConfig;
use crate::shared::project::ProjectRoot;

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
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;

    let request = DeployRequest {
        tenant_id: target.tenant_id,
        tenant_name: target.tenant_name,
        profile_name: profile.name.clone(),
        project_root: project.as_path().to_path_buf(),
        credentials: target.creds,
        hostname: target.hostname,
        secrets_path: ProfilePath::Secrets.resolve(profile_dir),
        signing_key_path: super::doctor::resolve_signing_key_path(&profile, profile_dir),
        options: DeployOptions {
            skip_push: args.skip_push,
            dry_run: args.dry_run,
            pre_sync: Some(PreSyncOptions {
                no_sync: args.no_sync,
                assume_yes: args.yes,
            }),
        },
    };

    let progress = CliDeployProgress::new(config);
    let report = DeployOrchestrator::new()
        .deploy(&request, &progress)
        .await?;

    if matches!(report.outcome, DeployOutcome::DryRun) {
        CliService::info("Dry run complete. No deployment performed.");
    }

    Ok(())
}

struct DeployTarget {
    tenant_id: TenantId,
    tenant_name: String,
    hostname: Option<String>,
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
        hostname: tenant.hostname.clone(),
        creds,
    })
}
