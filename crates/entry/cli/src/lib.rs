mod bootstrap;
pub mod cli_settings;
mod commands;
mod presentation;
pub mod requirements;
mod routing;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use systemprompt_logging::set_startup_mode;
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::DatabaseContext;

use crate::requirements::{CommandRequirements, HasRequirements};

#[derive(clap::Args)]
struct VerbosityOpts {
    #[arg(
        long,
        short = 'v',
        global = true,
        hide = true,
        help = "Increase verbosity"
    )]
    verbose: bool,

    #[arg(
        long,
        short = 'q',
        global = true,
        hide = true,
        conflicts_with = "verbose",
        help = "Suppress output"
    )]
    quiet: bool,

    #[arg(long, global = true, hide = true, help = "Debug logging")]
    debug: bool,
}

#[derive(clap::Args)]
struct OutputOpts {
    #[arg(long, global = true, hide = true, help = "JSON output")]
    json: bool,

    #[arg(
        long,
        global = true,
        hide = true,
        conflicts_with = "json",
        help = "YAML output"
    )]
    yaml: bool,
}

#[derive(clap::Args)]
struct DisplayOpts {
    #[arg(long, global = true, hide = true, help = "Disable colors")]
    no_color: bool,

    #[arg(long, global = true, hide = true, help = "Non-interactive mode")]
    non_interactive: bool,
}

#[derive(clap::Args)]
struct DatabaseOpts {
    #[arg(
        long,
        global = true,
        env = "SYSTEMPROMPT_DATABASE_URL",
        help = "Direct database URL (bypasses profile)"
    )]
    database_url: Option<String>,
}

#[derive(clap::Args)]
struct ProfileOpts {
    #[arg(
        long,
        global = true,
        help = "Profile name to use (overrides active session)"
    )]
    profile: Option<String>,
}

#[derive(Parser)]
#[command(name = "systemprompt")]
#[command(about = "Agent orchestration and AI operations")]
#[command(version = "0.1.0")]
#[command(
    before_help = "\x1b[38;5;208m</\x1b[1;37mSYSTEMPROMPT\x1b[38;5;208m.\x1b[0;37mio\x1b[38;5;\
                   208m>\x1b[0m"
)]
#[command(after_help = "\
GLOBAL OPTIONS (apply to all commands):
  -v, --verbose         Increase verbosity
  -q, --quiet           Suppress output
      --debug           Debug logging
      --json            JSON output
      --yaml            YAML output
      --no-color        Disable colors
      --non-interactive Non-interactive mode
      --database-url    Direct database URL (bypasses profile)
      --profile         Profile name to use (overrides active session)")]
struct Cli {
    #[command(flatten)]
    verbosity: VerbosityOpts,

    #[command(flatten)]
    output: OutputOpts,

    #[command(flatten)]
    display: DisplayOpts,

    #[command(flatten)]
    database: DatabaseOpts,

    #[command(flatten)]
    profile_opts: ProfileOpts,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        subcommand,
        about = "Core platform operations (content, files, contexts, skills)"
    )]
    Core(core::CoreCommands),

    #[command(
        subcommand,
        about = "Infrastructure management (services, db, jobs, logs, system)"
    )]
    Infra(infrastructure::InfraCommands),

    #[command(
        subcommand,
        about = "Administration (users, agents, config, setup, session)"
    )]
    Admin(admin::AdminCommands),

    #[command(subcommand, about = "Cloud deployment, sync, and setup")]
    Cloud(cloud::CloudCommands),

    #[command(subcommand, about = "Analytics and metrics reporting")]
    Analytics(analytics::AnalyticsCommands),

    #[command(subcommand, about = "Web service configuration management")]
    Web(web::WebCommands),

    #[command(subcommand, about = "Plugins, extensions, and MCP server management")]
    Plugins(plugins::PluginsCommands),

    #[command(subcommand, about = "Build MCP extensions")]
    Build(build::BuildCommands),
}

impl HasRequirements for Commands {
    fn requirements(&self) -> CommandRequirements {
        match self {
            Self::Cloud(cmd) => cmd.requirements(),
            Self::Plugins(cmd) => cmd.requirements(),
            Self::Admin(admin::AdminCommands::Setup(_) | admin::AdminCommands::Session(_)) => {
                CommandRequirements::NONE
            },
            Self::Build(_) => CommandRequirements::PROFILE_ONLY,
            Self::Infra(infrastructure::InfraCommands::System(_)) => {
                CommandRequirements::PROFILE_AND_SECRETS
            },
            _ => CommandRequirements::FULL,
        }
    }
}

const fn should_skip_validation(command: Option<&Commands>) -> bool {
    matches!(
        command,
        Some(Commands::Core(core::CoreCommands::Skills(
            core::skills::SkillsCommands::Create(_)
        )))
    )
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    set_startup_mode(cli.command.is_none());

    let cli_config = build_cli_config(&cli);
    cli_settings::set_global_config(cli_config.clone());

    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }

    if let Some(database_url) = cli.database.database_url.clone() {
        return run_with_database_url(cli.command, &cli_config, &database_url).await;
    }

    let reqs = cli
        .command
        .as_ref()
        .map_or(CommandRequirements::FULL, HasRequirements::requirements);

    if !reqs.database {
        systemprompt_logging::init_console_logging();
    }

    if reqs.profile {
        let profile_path = bootstrap::resolve_profile(cli_config.profile_override.as_deref())?;
        bootstrap::init_profile(&profile_path)?;
        bootstrap::init_credentials().await?;

        let profile = ProfileBootstrap::get()?;
        let is_cloud = profile.target.is_cloud();

        let is_fly_environment = std::env::var("FLY_APP_NAME").is_ok();

        if !is_fly_environment && should_check_remote_routing(cli.command.as_ref()) {
            match routing::determine_execution_target() {
                Ok(routing::ExecutionTarget::Remote {
                    hostname,
                    token,
                    context_id,
                }) => {
                    let args = reconstruct_args(&cli);
                    let exit_code =
                        routing::remote::execute_remote(&hostname, &token, &context_id, &args, 300)
                            .await?;
                    #[allow(clippy::exit)]
                    std::process::exit(exit_code);
                },
                Ok(routing::ExecutionTarget::Local) if is_cloud => {
                    bail!(
                        "Cloud profile '{}' requires remote execution but no tenant is configured.\n\
                         Ensure cloud.tenant_id is set and run 'systemprompt infra system login'.",
                        profile.name
                    );
                },
                Err(e) if is_cloud => {
                    bail!(
                        "Cloud profile '{}' requires remote execution but routing failed: {}\n\
                         Run 'systemprompt infra system login' to authenticate.",
                        profile.name,
                        e
                    );
                },
                _ => {},
            }
        } else if is_cloud && !is_fly_environment {
            bail!(
                "Cloud profile '{}' selected but this command doesn't support remote execution.\n\
                 Use a local profile with --profile <name>.",
                profile.name
            );
        }

        if reqs.secrets {
            bootstrap::init_secrets()?;
        }

        if reqs.paths {
            bootstrap::init_paths()?;
            if !should_skip_validation(cli.command.as_ref()) {
                bootstrap::run_validation()?;
            }
        }

        bootstrap::validate_cloud_credentials();
    }

    match cli.command {
        Some(Commands::Core(cmd)) => core::execute(cmd, &cli_config).await?,
        Some(Commands::Infra(cmd)) => infrastructure::execute(cmd, &cli_config).await?,
        Some(Commands::Admin(cmd)) => admin::execute(cmd, &cli_config).await?,
        Some(Commands::Cloud(cmd)) => cloud::execute(cmd, &cli_config).await?,
        Some(Commands::Analytics(cmd)) => analytics::execute(cmd, &cli_config).await?,
        Some(Commands::Web(cmd)) => web::execute(cmd)?,
        Some(Commands::Plugins(cmd)) => plugins::execute(cmd, &cli_config).await?,
        Some(Commands::Build(cmd)) => {
            build::execute(cmd, &cli_config)?;
        },
        None => {
            Cli::parse_from(["systemprompt", "--help"]);
        },
    }

    Ok(())
}

async fn run_with_database_url(
    command: Option<Commands>,
    config: &CliConfig,
    database_url: &str,
) -> Result<()> {
    let db_ctx = DatabaseContext::from_url(database_url)
        .await
        .context("Failed to connect to database")?;

    systemprompt_logging::init_logging(db_ctx.db_pool_arc());

    match command {
        Some(Commands::Core(cmd)) => core::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Infra(cmd)) => infrastructure::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Admin(cmd)) => admin::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Analytics(cmd)) => analytics::execute_with_db(cmd, &db_ctx, config).await,
        Some(_) => {
            bail!("This command requires full profile initialization. Remove --database-url flag.")
        },
        None => bail!("No subcommand provided. Use --help to see available commands."),
    }
}

fn build_cli_config(cli: &Cli) -> CliConfig {
    let mut cfg = CliConfig::new();

    if cli.verbosity.debug {
        cfg = cfg.with_verbosity(VerbosityLevel::Debug);
    } else if cli.verbosity.verbose {
        cfg = cfg.with_verbosity(VerbosityLevel::Verbose);
    } else if cli.verbosity.quiet {
        cfg = cfg.with_verbosity(VerbosityLevel::Quiet);
    }

    if cli.output.json {
        cfg = cfg.with_output_format(OutputFormat::Json);
    } else if cli.output.yaml {
        cfg = cfg.with_output_format(OutputFormat::Yaml);
    }

    if cli.display.no_color {
        cfg = cfg.with_color_mode(ColorMode::Never);
    }

    if cli.display.non_interactive {
        cfg = cfg.with_interactive(false);
    }

    cfg = cfg.with_profile_override(cli.profile_opts.profile.clone());

    cfg
}

const fn should_check_remote_routing(command: Option<&Commands>) -> bool {
    match command {
        Some(
            Commands::Admin(admin::AdminCommands::Session(_) | admin::AdminCommands::Setup(_))
            | Commands::Cloud(_)
            | Commands::Build(_)
            | Commands::Infra(infrastructure::InfraCommands::System(_)),
        )
        | None => false,
        Some(_) => true,
    }
}

fn reconstruct_args(cli: &Cli) -> Vec<String> {
    let mut args = Vec::new();

    if cli.verbosity.debug {
        args.push("--debug".to_string());
    } else if cli.verbosity.verbose {
        args.push("--verbose".to_string());
    } else if cli.verbosity.quiet {
        args.push("--quiet".to_string());
    }

    if cli.output.json {
        args.push("--json".to_string());
    } else if cli.output.yaml {
        args.push("--yaml".to_string());
    }

    if cli.display.no_color {
        args.push("--no-color".to_string());
    }

    if cli.display.non_interactive {
        args.push("--non-interactive".to_string());
    }

    if let Some(ref profile) = cli.profile_opts.profile {
        args.push("--profile".to_string());
        args.push(profile.clone());
    }

    let original_args: Vec<String> = std::env::args().skip(1).collect();
    for arg in &original_args {
        if !args.contains(arg)
            && !matches!(
                arg.as_str(),
                "--debug"
                    | "--verbose"
                    | "-v"
                    | "--quiet"
                    | "-q"
                    | "--json"
                    | "--yaml"
                    | "--no-color"
                    | "--non-interactive"
                    | "--profile"
            )
        {
            args.push(arg.clone());
        }
    }

    args
}
