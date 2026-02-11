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
use systemprompt_cloud::CredentialsBootstrapError;
use systemprompt_logging::{set_startup_mode, CliService};
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::DatabaseContext;

use crate::descriptor::{CommandDescriptor, DescribeCommand};

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
        if let Some(external_db_url) = init_profile_and_route(&cli, &desc, &cli_config).await? {
            return run_with_database_url(cli.command, &cli_config, &external_db_url).await;
        }
    }

    dispatch_command(cli.command, &cli_config).await
}

async fn init_profile_and_route(
    cli: &args::Cli,
    desc: &CommandDescriptor,
    cli_config: &CliConfig,
) -> Result<Option<String>> {
    let profile_path = bootstrap::resolve_profile(cli_config.profile_override.as_deref())?;
    bootstrap::init_profile(&profile_path)?;

    let profile = ProfileBootstrap::get()?;

    if cli_config.output_format == OutputFormat::Table
        && cli_config.verbosity != VerbosityLevel::Quiet
    {
        let tenant = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_deref());
        CliService::profile_banner(&profile.name, profile.target.is_cloud(), tenant);
    }

    let is_cloud = profile.target.is_cloud();
    let env = environment::ExecutionEnvironment::detect();
    let has_export = has_local_export_flag(cli.command.as_ref());

    if !env.is_fly && desc.remote_eligible && !has_export {
        try_remote_routing(cli, profile).await?;
    } else if has_export && is_cloud && !profile.database.external_db_access {
        bail!(
            "Export with cloud profile '{}' requires external database access.\nEnable \
             external_db_access in the profile or use a local profile.",
            profile.name
        );
    } else if is_cloud
        && !env.is_fly
        && !profile.database.external_db_access
        && !matches!(
            cli.command.as_ref(),
            Some(
                args::Commands::Cloud(_) | args::Commands::Admin(admin::AdminCommands::Session(_))
            )
        )
    {
        bail!(
            "Cloud profile '{}' selected but this command doesn't support remote execution.\nUse \
             a local profile with --profile <name> or enable external database access.",
            profile.name
        );
    }

    if !is_cloud || profile.database.external_db_access {
        if let Err(e) = bootstrap::init_credentials().await {
            let is_file_not_found = e
                .downcast_ref::<CredentialsBootstrapError>()
                .is_some_and(|ce| matches!(ce, CredentialsBootstrapError::FileNotFound { .. }));

            if is_file_not_found {
                tracing::debug!(error = %e, "Credentials file not found, continuing in local-only mode");
            } else {
                return Err(e.context("Credential initialization failed"));
            }
        }
    }

    if desc.secrets {
        bootstrap::init_secrets()?;
    }

    if is_cloud && profile.database.external_db_access && desc.paths && !env.is_fly {
        let secrets = SecretsBootstrap::get()
            .map_err(|e| anyhow::anyhow!("Secrets required for external DB access: {}", e))?;
        let db_url = secrets.effective_database_url(true).to_string();
        return Ok(Some(db_url));
    }

    if desc.paths {
        bootstrap::init_paths()?;
        if !desc.skip_validation {
            bootstrap::run_validation()?;
        }
    }

    if !is_cloud {
        bootstrap::validate_cloud_credentials(&env);
    }

    Ok(None)
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
