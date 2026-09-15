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

use anyhow::{Context, Result, bail};
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};

use super::{args, bootstrap};
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

/// Where a command runs once the routing target is known.
#[derive(Debug, PartialEq, Eq)]
pub enum RoutingDecision {
    ExecuteRemote {
        hostname: String,
        token: systemprompt_identifiers::SessionToken,
        context: systemprompt_identifiers::ContextId,
    },
    ContinueLocal,
}

pub(super) async fn bootstrap_profile(
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
    env: &EnvOverrides,
) -> Result<BootstrapOutcome> {
    let has_export = args::has_local_export_flag(cli.command.as_ref());
    let ctx = bootstrap::resolve_and_display_profile(cli_config, env, has_export)?;

    require_explicit_cloud_profile(ProfileBootstrap::get()?, ctx.source, desc)?;
    if enforce_routing_policy(&ctx, cli, desc, cli_config).await?
        == BootstrapOutcome::RemoteExecuted
    {
        return Ok(BootstrapOutcome::RemoteExecuted);
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
        let profile = ProfileBootstrap::get()?;
        return try_remote_routing(cli, profile, cli_config, class).await;
    }

    if ctx.has_export && ctx.is_cloud && !ctx.external_db_access {
        bail!(
            "Export with cloud profile '{}' requires external database access.\nEnable \
             external_db_access in the profile or use a local profile.",
            ctx.profile_name
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
            ctx.profile_name
        );
    }

    Ok(BootstrapOutcome::ContinueLocal)
}

pub fn require_explicit_cloud_profile(
    profile: &systemprompt_models::Profile,
    source: ProfileSource,
    desc: &CommandDescriptor,
) -> Result<()> {
    if !desc.requires_explicit_cloud_profile() || source.is_explicit() {
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
) -> Result<BootstrapOutcome> {
    if needs_cloud || (ctx.is_cloud && ctx.external_db_access) {
        bootstrap::init_credentials_gracefully(needs_cloud).await?;
    }

    if desc.secrets() {
        bootstrap::init_secrets().await?;
    }

    if ctx.is_cloud && ctx.external_db_access && desc.paths() && !ctx.env.is_deployment_host {
        let secrets = SecretsBootstrap::get().context("Secrets required for external DB access")?;
        let db_url = secrets.effective_database_url(true).to_owned();
        return Ok(BootstrapOutcome::ExternalDbUrl(db_url));
    }

    if desc.paths() {
        bootstrap::init_paths(desc.discovers_models()).await?;
        if !desc.skip_validation() {
            bootstrap::run_validation()?;
        }
    }

    if !ctx.is_cloud {
        bootstrap::validate_cloud_credentials(&ctx.env);
    }

    Ok(BootstrapOutcome::ContinueLocal)
}

async fn try_remote_routing(
    cli: &args::Cli,
    profile: &systemprompt_models::Profile,
    cli_config: &CliConfig,
    class: RoutingClass,
) -> Result<BootstrapOutcome> {
    use super::routing;

    let decision = decide_routing(routing::determine_execution_target(), profile, class)?;
    let RoutingDecision::ExecuteRemote {
        hostname,
        token,
        context,
    } = decision
    else {
        return Ok(BootstrapOutcome::ContinueLocal);
    };

    confirm_remote_job_run(cli, cli_config, &profile.name, &hostname)?;
    let args = args::reconstruct_args(cli);
    let exit_code = routing::execute_remote(&hostname, &token, &context, &args, 300).await?;
    if exit_code != 0 {
        bail!("Remote command exited with code {}", exit_code);
    }
    Ok(BootstrapOutcome::RemoteExecuted)
}

pub fn decide_routing(
    target: Result<super::routing::ExecutionTarget>,
    profile: &systemprompt_models::Profile,
    class: RoutingClass,
) -> Result<RoutingDecision> {
    use super::routing::ExecutionTarget;

    let is_cloud = profile.target.is_cloud();
    match target {
        Ok(ExecutionTarget::Remote {
            hostname,
            token,
            context,
        }) => Ok(RoutingDecision::ExecuteRemote {
            hostname,
            token,
            context,
        }),
        Ok(ExecutionTarget::Local) if is_cloud => {
            allow_local_execution(profile, class, "no tenant is configured")?;
            Ok(RoutingDecision::ContinueLocal)
        },
        Err(e) if is_cloud => {
            allow_local_execution(profile, class, &format!("routing failed: {}", e))?;
            Ok(RoutingDecision::ContinueLocal)
        },
        Ok(ExecutionTarget::Local) => Ok(RoutingDecision::ContinueLocal),
        Err(e) => {
            tracing::debug!(error = %e, "Routing failed on a local profile; continuing locally");
            Ok(RoutingDecision::ContinueLocal)
        },
    }
}

pub fn confirm_remote_job_run(
    cli: &args::Cli,
    cli_config: &CliConfig,
    profile_name: &str,
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
        run_args.job_names.join(", ")
    };

    let message = format!(
        "Run {selection} against REMOTE profile '{profile_name}' ({hostname})?\nPass --profile \
         <local-profile> to target a local environment instead. Continue?"
    );

    interactive::require_confirmation(
        &interactive::DialoguerPrompter,
        &message,
        run_args.yes,
        cli_config,
    )
}

pub fn allow_local_execution(
    profile: &systemprompt_models::Profile,
    class: RoutingClass,
    reason: &str,
) -> Result<()> {
    if profile.database.external_db_access {
        tracing::debug!(
            profile_name = %profile.name,
            reason = reason,
            "Cloud profile allowing local execution via external_db_access"
        );
        return Ok(());
    }

    if class == RoutingClass::ReadOnly {
        tracing::warn!(
            profile_name = %profile.name,
            reason = reason,
            "Cloud profile could not route remotely; reading local data instead"
        );
        return Ok(());
    }

    bail!(
        "Cloud profile '{}' requires remote execution but {}.\n{}",
        profile.name,
        reason,
        remediation_for(reason)
    )
}

pub fn remediation_for(reason: &str) -> &'static str {
    if reason.contains("load tenants") || reason.contains("tenant") {
        "Run 'systemprompt cloud tenant list' to sync the tenant store, and check you are in the \
         project directory this profile belongs to."
    } else {
        "Run 'systemprompt admin session login' to authenticate."
    }
}
