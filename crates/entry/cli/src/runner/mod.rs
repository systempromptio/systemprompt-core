//! CLI runtime entry point and bootstrap helpers.
//!
//! Owns argument parsing (`args`), profile/secrets bootstrap (`bootstrap`),
//! and cloud routing (`routing`). The public surface is just [`run`]; every
//! other symbol stays scoped to the runner subtree.

mod args;
mod bootstrap;
mod db_url;
mod profile_routing;
mod routing;
mod structured_output;

use anyhow::{Context, Result, bail};
use clap::Parser;
use systemprompt_logging::set_startup_mode;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::{CliConfig, OutputFormat};
use crate::commands::{admin, analytics, cloud, core, infrastructure, plugins, web};
use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::env_overrides::EnvOverrides;

pub async fn run() -> Result<()> {
    let outcome = Box::pin(run_inner()).await;
    structured_output::finalize(&outcome);
    outcome
}

async fn run_inner() -> Result<()> {
    let cli = args::Cli::parse();

    set_startup_mode(cli.command.is_none());

    let env = EnvOverrides::from_process_env();
    let cli_config = args::build_cli_config(&cli, &env);
    systemprompt_logging::set_structured_output(cli_config.output_format() != OutputFormat::Table);

    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }

    if let Some(database_url) = cli.database.database_url.clone() {
        match cli.command.as_ref().map(args::Commands::db_url_routing) {
            Some(db_url::DbUrlRouting::Direct) => {
                return Box::pin(run_with_database_url(
                    cli.command,
                    cli_config,
                    env,
                    &database_url,
                ))
                .await;
            },
            Some(db_url::DbUrlRouting::Unsupported) => bail!(
                "This command cannot run with --database-url; it requires full profile \
                 initialization. Remove --database-url."
            ),
            Some(db_url::DbUrlRouting::ProfileDriven) | None => {},
        }
    }

    let desc = cli
        .command
        .as_ref()
        .map_or(CommandDescriptor::FULL, DescribeCommand::descriptor);

    if !desc.database() {
        let effective_level = resolve_log_level(&cli_config, &env);
        systemprompt_logging::init_console_logging_with_level(effective_level.as_deref());
    }

    if desc.profile()
        && let Some(external_db_url) =
            profile_routing::bootstrap_profile(&cli, &desc, &cli_config, &env).await?
    {
        return Box::pin(run_with_database_url(
            cli.command,
            cli_config,
            env,
            &external_db_url,
        ))
        .await;
    }

    let ctx = CommandContext::new(cli_config, env);
    dispatch_command(cli.command, &ctx).await
}

fn resolve_log_level(cli_config: &CliConfig, env: &EnvOverrides) -> Option<String> {
    if env.rust_log.is_some() {
        return None;
    }

    if let Some(level) = cli_config.verbosity.as_tracing_filter() {
        return Some(level.to_owned());
    }

    if let Ok(profile_path) =
        bootstrap::resolve_profile(cli_config.profile_override.as_deref(), env)
        && let Some(log_level) = bootstrap::try_load_log_level(&profile_path)
    {
        return Some(log_level.as_tracing_filter().to_owned());
    }

    Some("warn".to_owned())
}

async fn dispatch_command(command: Option<args::Commands>, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() {
        match &command {
            Some(
                args::Commands::Core(_)
                | args::Commands::Infra(_)
                | args::Commands::Admin(_)
                | args::Commands::Analytics(_)
                | args::Commands::Cloud(cloud::CloudCommands::Db(_)),
            ) => {},
            Some(_) => bail!(
                "This command requires full profile initialization. Remove --database-url flag."
            ),
            None => bail!("No subcommand provided. Use --help to see available commands."),
        }
    }

    match command {
        Some(args::Commands::Core(cmd)) => core::execute(cmd, ctx).await?,
        Some(args::Commands::Infra(cmd)) => infrastructure::execute(cmd, ctx).await?,
        Some(args::Commands::Admin(cmd)) => admin::execute(cmd, ctx).await?,
        Some(args::Commands::Cloud(cmd)) => cloud::execute(cmd, ctx).await?,
        Some(args::Commands::Analytics(cmd)) => analytics::execute(cmd, ctx).await?,
        Some(args::Commands::Web(cmd)) => web::execute(cmd, ctx)?,
        Some(args::Commands::Plugins(cmd)) => plugins::execute(cmd, ctx).await?,
        Some(args::Commands::Build(cmd)) => {
            crate::commands::build::execute(cmd, ctx)?;
        },
        None => {
            args::Cli::parse_from(["systemprompt", "--help"]);
        },
    }

    Ok(())
}

async fn run_with_database_url(
    command: Option<args::Commands>,
    cli_config: CliConfig,
    env: EnvOverrides,
    database_url: &str,
) -> Result<()> {
    let db_ctx = DatabaseContext::from_url(database_url)
        .await
        .context("Failed to connect to database")?;

    systemprompt_logging::init_logging(db_ctx.db_pool_arc());

    let ctx = CommandContext::with_database(cli_config, env, db_ctx, database_url.to_owned());
    dispatch_command(command, &ctx).await
}
