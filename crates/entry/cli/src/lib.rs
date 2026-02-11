mod args;
mod bootstrap;
pub mod cli_settings;
mod commands;
pub mod descriptor;
pub mod environment;
pub mod interactive;
pub mod paths;
mod presentation;
mod routing;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};

use anyhow::{bail, Context, Result};
use clap::Parser;
use systemprompt_logging::set_startup_mode;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::DatabaseContext;

use crate::descriptor::{CommandDescriptor, DescribeCommand};

enum RoutingAction {
    ContinueLocal,
    ExternalDbUrl(String),
}

fn has_local_export_flag(command: Option<&args::Commands>) -> bool {
    let is_analytics = matches!(command, Some(args::Commands::Analytics(_)));
    if !is_analytics {
        return false;
    }
    std::env::args().any(|arg| arg == "--export" || arg.starts_with("--export="))
}

pub async fn run() -> Result<()> {
    let cli = args::Cli::parse();

    set_startup_mode(cli.command.is_none());

    let cli_config = args::build_cli_config(&cli);
    cli_settings::set_global_config(cli_config.clone());

    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }

    if let Some(database_url) = cli.database.database_url.clone() {
        return run_with_database_url(cli.command, &cli_config, &database_url).await;
    }

    let desc = cli
        .command
        .as_ref()
        .map_or(CommandDescriptor::FULL, DescribeCommand::descriptor);

    if !desc.database {
        let effective_level = resolve_log_level(&cli_config);
        systemprompt_logging::init_console_logging_with_level(effective_level.as_deref());
    }

    if desc.profile {
        if let Some(external_db_url) = bootstrap_profile(&cli, &desc, &cli_config).await? {
            return run_with_database_url(cli.command, &cli_config, &external_db_url).await;
        }
    }

    dispatch_command(cli.command, &cli_config).await
}

async fn bootstrap_profile(
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
) -> Result<Option<String>> {
    let has_export = has_local_export_flag(cli.command.as_ref());
    let ctx = bootstrap::resolve_and_display_profile(cli_config, has_export)?;

    enforce_routing_policy(&ctx, cli, desc).await?;

    match initialize_post_routing(&ctx, desc).await? {
        RoutingAction::ExternalDbUrl(url) => Ok(Some(url)),
        RoutingAction::ContinueLocal => Ok(None),
    }
}

async fn enforce_routing_policy(
    ctx: &bootstrap::ProfileContext,
    cli: &args::Cli,
    desc: &CommandDescriptor,
) -> Result<()> {
    if !ctx.env.is_fly && desc.remote_eligible && !ctx.has_export {
        let profile = ProfileBootstrap::get()?;
        try_remote_routing(cli, profile).await?;
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
) -> Result<RoutingAction> {
    if !ctx.is_cloud || ctx.external_db_access {
        bootstrap::init_credentials_gracefully().await?;
    }

    if desc.secrets {
        bootstrap::init_secrets()?;
    }

    if ctx.is_cloud && ctx.external_db_access && desc.paths && !ctx.env.is_fly {
        let secrets = SecretsBootstrap::get()
            .map_err(|e| anyhow::anyhow!("Secrets required for external DB access: {}", e))?;
        let db_url = secrets.effective_database_url(true).to_string();
        return Ok(RoutingAction::ExternalDbUrl(db_url));
    }

    if desc.paths {
        bootstrap::init_paths()?;
        if !desc.skip_validation {
            bootstrap::run_validation()?;
        }
    }

    if !ctx.is_cloud {
        bootstrap::validate_cloud_credentials(&ctx.env);
    }

    Ok(RoutingAction::ContinueLocal)
}

async fn try_remote_routing(cli: &args::Cli, profile: &systemprompt_models::Profile) -> Result<()> {
    let is_cloud = profile.target.is_cloud();

    match routing::determine_execution_target() {
        Ok(routing::ExecutionTarget::Remote {
            hostname,
            token,
            context_id,
        }) => {
            let args = args::reconstruct_args(cli);
            let exit_code =
                routing::remote::execute_remote(&hostname, &token, &context_id, &args, 300).await?;
            #[allow(clippy::exit)]
            std::process::exit(exit_code);
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

fn resolve_log_level(cli_config: &CliConfig) -> Option<String> {
    if std::env::var("RUST_LOG").is_ok() {
        return None;
    }

    if let Some(level) = cli_config.verbosity.as_tracing_filter() {
        return Some(level.to_string());
    }

    if let Ok(profile_path) = bootstrap::resolve_profile(cli_config.profile_override.as_deref()) {
        if let Some(log_level) = bootstrap::try_load_log_level(&profile_path) {
            return Some(log_level.as_tracing_filter().to_string());
        }
    }

    None
}

async fn dispatch_command(command: Option<args::Commands>, config: &CliConfig) -> Result<()> {
    match command {
        Some(args::Commands::Core(cmd)) => core::execute(cmd, config).await?,
        Some(args::Commands::Infra(cmd)) => infrastructure::execute(cmd, config).await?,
        Some(args::Commands::Admin(cmd)) => admin::execute(cmd, config).await?,
        Some(args::Commands::Cloud(cmd)) => cloud::execute(cmd, config).await?,
        Some(args::Commands::Analytics(cmd)) => analytics::execute(cmd, config).await?,
        Some(args::Commands::Web(cmd)) => web::execute(cmd)?,
        Some(args::Commands::Plugins(cmd)) => plugins::execute(cmd, config).await?,
        Some(args::Commands::Build(cmd)) => {
            build::execute(cmd, config)?;
        },
        None => {
            args::Cli::parse_from(["systemprompt", "--help"]);
        },
    }

    Ok(())
}

async fn run_with_database_url(
    command: Option<args::Commands>,
    config: &CliConfig,
    database_url: &str,
) -> Result<()> {
    let db_ctx = DatabaseContext::from_url(database_url)
        .await
        .context("Failed to connect to database")?;

    systemprompt_logging::init_logging(db_ctx.db_pool_arc());

    match command {
        Some(args::Commands::Core(cmd)) => core::execute_with_db(cmd, &db_ctx, config).await,
        Some(args::Commands::Infra(cmd)) => {
            infrastructure::execute_with_db(cmd, &db_ctx, config).await
        },
        Some(args::Commands::Admin(cmd)) => admin::execute_with_db(cmd, &db_ctx, config).await,
        Some(args::Commands::Analytics(cmd)) => {
            analytics::execute_with_db(cmd, &db_ctx, config).await
        },
        Some(args::Commands::Cloud(cloud::CloudCommands::Db(cmd))) => {
            cloud::db::execute_with_database_url(cmd, database_url, config).await
        },
        Some(_) => {
            bail!("This command requires full profile initialization. Remove --database-url flag.")
        },
        None => bail!("No subcommand provided. Use --help to see available commands."),
    }
}
