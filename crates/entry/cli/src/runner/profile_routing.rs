//! Profile bootstrap and cloud-routing policy for the CLI runner.
//!
//! Resolves the active profile, enforces whether a command may run locally or
//! must route to a remote tenant, and initialises credentials, secrets, and
//! paths accordingly. The single entry point is [`bootstrap_profile`]; it
//! returns an external database URL when the command should reconnect against
//! a cloud-issued database instead of continuing the local boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};

use super::{args, bootstrap};
use crate::cli_settings::CliConfig;
use crate::commands::{admin, infrastructure};
use crate::descriptor::CommandDescriptor;
use crate::env_overrides::EnvOverrides;
use crate::interactive;

enum RoutingAction {
    ContinueLocal,
    ExternalDbUrl(String),
}

pub(super) async fn bootstrap_profile(
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
    env: &EnvOverrides,
) -> Result<Option<String>> {
    let has_export = args::has_local_export_flag(cli.command.as_ref());
    let ctx = bootstrap::resolve_and_display_profile(cli_config, env, has_export)?;

    enforce_routing_policy(&ctx, cli, desc, cli_config).await?;

    let needs_cloud = is_cloud_bypass_command(cli.command.as_ref());
    match initialize_post_routing(&ctx, desc, needs_cloud).await? {
        RoutingAction::ExternalDbUrl(url) => Ok(Some(url)),
        RoutingAction::ContinueLocal => Ok(None),
    }
}

async fn enforce_routing_policy(
    ctx: &bootstrap::ProfileContext,
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
) -> Result<()> {
    if !ctx.env.is_fly && desc.remote_eligible() && !ctx.has_export {
        let profile = ProfileBootstrap::get()?;
        try_remote_routing(cli, profile, cli_config).await?;
        return Ok(());
    }

    if ctx.has_export && ctx.is_cloud && !ctx.external_db_access {
        bail!(
            "Export with cloud profile '{}' requires external database access.\nEnable \
             external_db_access in the profile or use a local profile.",
            ctx.profile_name
        );
    }

    if ctx.is_cloud
        && !ctx.env.is_fly
        && !ctx.external_db_access
        && !is_cloud_bypass_command(cli.command.as_ref())
    {
        bail!(
            "Cloud profile '{}' selected but this command doesn't support remote execution.\nUse \
             a local profile with --profile <name> or enable external database access.",
            ctx.profile_name
        );
    }

    Ok(())
}

const fn is_cloud_bypass_command(command: Option<&args::Commands>) -> bool {
    matches!(
        command,
        Some(args::Commands::Cloud(_) | args::Commands::Admin(admin::AdminCommands::Session(_)))
    )
}

async fn initialize_post_routing(
    ctx: &bootstrap::ProfileContext,
    desc: &CommandDescriptor,
    needs_cloud: bool,
) -> Result<RoutingAction> {
    // Why: only commands that hit the cloud control plane should consult
    // cloud credentials. `external_db_access` is preserved because that
    // path resolves the DB URL from cloud-issued creds even from a local
    // CLI.
    if needs_cloud || (ctx.is_cloud && ctx.external_db_access) {
        bootstrap::init_credentials_gracefully(needs_cloud).await?;
    }

    if desc.secrets() {
        bootstrap::init_secrets()?;
    }

    if ctx.is_cloud && ctx.external_db_access && desc.paths() && !ctx.env.is_fly {
        let secrets = SecretsBootstrap::get().context("Secrets required for external DB access")?;
        let db_url = secrets.effective_database_url(true).to_owned();
        return Ok(RoutingAction::ExternalDbUrl(db_url));
    }

    if desc.paths() {
        bootstrap::init_paths()?;
        if !desc.skip_validation() {
            bootstrap::run_validation()?;
        }
    }

    if !ctx.is_cloud {
        bootstrap::validate_cloud_credentials(&ctx.env);
    }

    Ok(RoutingAction::ContinueLocal)
}

async fn try_remote_routing(
    cli: &args::Cli,
    profile: &systemprompt_models::Profile,
    cli_config: &CliConfig,
) -> Result<()> {
    use super::routing;

    let is_cloud = profile.target.is_cloud();

    match routing::determine_execution_target() {
        Ok(routing::ExecutionTarget::Remote {
            hostname,
            token,
            context,
        }) => {
            confirm_remote_job_run(cli, cli_config, &profile.name, &hostname)?;
            let args = args::reconstruct_args(cli);
            let exit_code =
                routing::execute_remote(&hostname, token.as_str(), context.as_str(), &args, 300)
                    .await?;
            if exit_code != 0 {
                bail!("Remote command exited with code {}", exit_code);
            }
            return Ok(());
        },
        Ok(routing::ExecutionTarget::Local) if is_cloud => {
            require_external_db_access(profile, "no tenant is configured")?;
        },
        Err(e) if is_cloud => {
            require_external_db_access(profile, &format!("routing failed: {}", e))?;
        },
        _ => {},
    }

    Ok(())
}

fn confirm_remote_job_run(
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

fn require_external_db_access(profile: &systemprompt_models::Profile, reason: &str) -> Result<()> {
    if profile.database.external_db_access {
        tracing::debug!(
            profile_name = %profile.name,
            reason = reason,
            "Cloud profile allowing local execution via external_db_access"
        );
        Ok(())
    } else {
        bail!(
            "Cloud profile '{}' requires remote execution but {}.\nRun 'systemprompt admin \
             session login' to authenticate.",
            profile.name,
            reason
        )
    }
}
