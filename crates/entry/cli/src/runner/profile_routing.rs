//! Profile bootstrap and cloud-routing policy for the CLI runner.
//!
//! Resolves the active profile, enforces whether a command may run locally or
//! must route to a remote tenant, and initialises credentials, secrets, and
//! paths accordingly. The single entry point is `bootstrap_profile`; its
//! [`BootstrapOutcome`] tells the runner whether the command already ran on
//! the remote tenant, should continue locally, or should reconnect against a
//! cloud-issued database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use systemprompt_cloud::CloudCredentials;
use systemprompt_config::LoadedProfile;
use systemprompt_identifiers::JobName;
use systemprompt_loader::BootContext;

use super::routing_decision::{RoutingDecision, decide_routing};
use super::{args, bootstrap};
use systemprompt_loader::ServicesState;

use crate::cli_settings::CliConfig;
use crate::commands::{admin, infrastructure};
use crate::descriptor::{CommandDescriptor, RoutingClass};
use crate::env_overrides::EnvOverrides;
use crate::interactive;
use crate::shared::ProfileSource;

/// What the runner does once the profile is bootstrapped.
///
/// `RemoteExecuted` means the command has already run on the remote tenant
/// and its output has been streamed; the runner must not dispatch it again
/// locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    RemoteExecuted,
    ContinueLocal,
    ExternalDbUrl(String),
}

pub(super) struct Bootstrapped {
    pub outcome: BootstrapOutcome,
    pub credentials: Option<CloudCredentials>,
    pub services: Option<ServicesState>,
    pub profile: Option<Arc<LoadedProfile>>,
    pub boot: Option<BootContext>,
}

impl Bootstrapped {
    const fn new(outcome: BootstrapOutcome, credentials: Option<CloudCredentials>) -> Self {
        Self {
            outcome,
            credentials,
            services: None,
            profile: None,
            boot: None,
        }
    }

    fn with_services(mut self, services: Option<ServicesState>) -> Self {
        self.services = services;
        self
    }

    fn with_profile(mut self, profile: Arc<LoadedProfile>, boot: Option<BootContext>) -> Self {
        self.profile = Some(profile);
        self.boot = boot;
        self
    }
}

pub(super) async fn bootstrap_profile(
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
    env: &EnvOverrides,
) -> Result<Bootstrapped> {
    let has_export = args::has_local_export_flag(cli.command.as_ref());
    let ctx = bootstrap::resolve_and_display_profile(cli_config, env, has_export)?;

    require_explicit_cloud_profile(ctx.loaded.profile(), ctx.source, desc)?;
    if enforce_routing_policy(&ctx, cli, desc, cli_config).await?
        == BootstrapOutcome::RemoteExecuted
    {
        return Ok(Bootstrapped::new(BootstrapOutcome::RemoteExecuted, None));
    }

    let needs_cloud = is_cloud_bypass_command(cli.command.as_ref());
    initialize_post_routing(&ctx, desc, needs_cloud).await
}

async fn enforce_routing_policy(
    ctx: &bootstrap::ProfileContext,
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
) -> Result<BootstrapOutcome> {
    let class = desc.routing_class();
    if !ctx.env.is_deployment_host && class != RoutingClass::LocalOnly && !ctx.has_export {
        return try_remote_routing(cli, ctx.loaded.profile(), cli_config, desc).await;
    }

    if ctx.has_export && ctx.is_cloud && !ctx.external_db_access {
        bail!(
            "Export with cloud profile '{}' requires external database access.\nEnable \
             external_db_access in the profile or use a local profile.",
            ctx.loaded.name
        );
    }

    if ctx.is_cloud
        && !ctx.env.is_deployment_host
        && !ctx.external_db_access
        && !is_cloud_bypass_command(cli.command.as_ref())
    {
        bail!(
            "Cloud profile '{}' selected but this command doesn't support remote execution.\nUse \
             a local profile with --profile <name> or enable external database access.",
            ctx.loaded.name
        );
    }

    Ok(BootstrapOutcome::ContinueLocal)
}

pub fn require_explicit_cloud_profile(
    profile: &systemprompt_manifest::Profile,
    source: ProfileSource,
    desc: &CommandDescriptor,
) -> Result<()> {
    if !desc.is_destructive() || source.is_explicit() {
        return Ok(());
    }

    let has_tenant = profile
        .cloud
        .as_ref()
        .is_some_and(|cloud| cloud.tenant_id.is_some());
    if !(has_tenant || profile.target.is_cloud()) {
        return Ok(());
    }

    bail!(
        "profile `{}` is a cloud profile selected implicitly (stored session or directory \
         discovery); pass `--profile {}` or set SYSTEMPROMPT_PROFILE to target it",
        profile.name,
        profile.name
    )
}

pub const fn is_cloud_bypass_command(command: Option<&args::Commands>) -> bool {
    matches!(
        command,
        Some(args::Commands::Cloud(_) | args::Commands::Admin(admin::AdminCommands::Session(_)))
    )
}

async fn initialize_post_routing(
    ctx: &bootstrap::ProfileContext,
    desc: &CommandDescriptor,
    needs_cloud: bool,
) -> Result<Bootstrapped> {
    let credentials = if needs_cloud || (ctx.is_cloud && ctx.external_db_access) {
        bootstrap::init_credentials_gracefully(needs_cloud).await?
    } else {
        None
    };

    let boot = if desc.secrets() {
        Some(bootstrap::init_secrets(&ctx.loaded).await?)
    } else {
        None
    };

    if ctx.is_cloud && ctx.external_db_access && desc.paths() && !ctx.env.is_deployment_host {
        let boot = boot
            .as_ref()
            .context("Secrets required for external DB access")?;
        let db_url = boot.secrets().get().effective_database_url(true).to_owned();
        return Ok(
            Bootstrapped::new(BootstrapOutcome::ExternalDbUrl(db_url), credentials)
                .with_profile(Arc::clone(&ctx.loaded), Some(boot.clone())),
        );
    }

    let mut services = None;
    if desc.paths() {
        let boot = boot
            .as_ref()
            .context("Secrets required to resolve the services paths")?;
        services = Some(bootstrap::init_paths(boot, desc.discovers_models()).await?);
        if !desc.skip_validation() {
            bootstrap::run_validation(boot)?;
        }
    }

    if !ctx.is_cloud {
        bootstrap::validate_cloud_credentials(&ctx.env, credentials.as_ref(), ctx.loaded.profile());
    }

    Ok(
        Bootstrapped::new(BootstrapOutcome::ContinueLocal, credentials)
            .with_services(services)
            .with_profile(Arc::clone(&ctx.loaded), boot),
    )
}

async fn try_remote_routing(
    cli: &args::Cli,
    profile: &systemprompt_manifest::Profile,
    cli_config: &CliConfig,
    desc: &CommandDescriptor,
) -> Result<BootstrapOutcome> {
    use super::routing;

    let target = match routing::determine_execution_target(profile) {
        Ok(target) => routing::authorize_target(target).await,
        Err(e) => Err(e),
    };
    let decision = decide_routing(target, profile, desc.routing_class(), desc.data_impact())?;
    let RoutingDecision::ExecuteRemote {
        hostname,
        token,
        context,
    } = decision
    else {
        return Ok(BootstrapOutcome::ContinueLocal);
    };

    confirm_remote_job_run(cli, cli_config, profile, &hostname)?;
    let args = args::reconstruct_args();
    let exit_code = routing::execute_remote(&hostname, &token, &context, &args, 300).await?;
    if exit_code != 0 {
        bail!("Remote command exited with code {}", exit_code);
    }
    Ok(BootstrapOutcome::RemoteExecuted)
}

pub fn confirm_remote_job_run(
    cli: &args::Cli,
    cli_config: &CliConfig,
    profile: &systemprompt_manifest::Profile,
    hostname: &str,
) -> Result<()> {
    let Some(args::Commands::Infra(infrastructure::InfraCommands::Jobs(
        infrastructure::jobs::JobsCommands::Run(run_args),
    ))) = cli.command.as_ref()
    else {
        return Ok(());
    };

    let selection = if run_args.all {
        "all jobs".to_owned()
    } else if let Some(tag) = &run_args.tag {
        format!("jobs tagged '{tag}'")
    } else {
        run_args
            .job_names
            .iter()
            .map(JobName::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };

    let message = format!(
        "Run {selection} against REMOTE profile '{}' ({hostname})?\nPass --profile \
         <local-profile> to target a local environment instead. Continue?",
        profile.name
    );

    interactive::require_confirmation(
        &interactive::DialoguerPrompter,
        &message,
        run_args.yes,
        cli_config,
    )
}
