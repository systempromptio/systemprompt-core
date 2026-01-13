pub mod cli_settings;
mod commands;
mod presentation;
pub mod shared;
mod tui;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{
    agents, build, cloud, db, files, jobs, logs, mcp, services, setup, skills, users,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_core_files::FilesConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::profile::CloudValidationMode;
use systemprompt_models::{AppPaths, Config, ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::{
    display_validation_report, display_validation_warnings, StartupValidator,
};

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
      --non-interactive Non-interactive mode")]
struct Cli {
    #[command(flatten)]
    verbosity: VerbosityOpts,

    #[command(flatten)]
    output: OutputOpts,

    #[command(flatten)]
    display: DisplayOpts,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        subcommand,
        about = "Service lifecycle management (start, stop, status)"
    )]
    Services(services::ServicesCommands),

    #[command(subcommand, about = "Database operations and administration")]
    Db(db::DbCommands),

    #[command(subcommand, about = "Background jobs and scheduling")]
    Jobs(jobs::JobsCommands),

    #[command(subcommand, about = "Cloud deployment, sync, and setup")]
    Cloud(cloud::CloudCommands),

    #[command(subcommand, about = "Agent management")]
    Agents(agents::AgentsCommands),

    #[command(subcommand, about = "MCP server management")]
    Mcp(mcp::McpCommands),

    #[command(subcommand, about = "Log streaming and tracing")]
    Logs(logs::LogsCommands),

    #[command(subcommand, about = "Build MCP extensions")]
    Build(build::BuildCommands),

    #[command(subcommand, about = "Skill management and database sync")]
    Skills(skills::SkillsCommands),

    #[command(subcommand, about = "User management and IP banning")]
    Users(users::UsersCommands),

    #[command(subcommand, about = "File management and uploads")]
    Files(files::FilesCommands),

    #[command(about = "Interactive setup wizard for local development environment")]
    Setup(setup::SetupArgs),
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    let cli_config = build_cli_config(&cli);
    cli_settings::set_global_config(cli_config);

    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }

    let (requires_profile, requires_secrets) = match &cli.command {
        Some(Commands::Cloud(cmd)) => (cmd.requires_profile(), cmd.requires_secrets()),
        Some(Commands::Setup(_)) => (false, false),
        Some(Commands::Build(_)) => (true, false),
        Some(_) | None => (true, true),
    };

    if requires_profile {
        ProfileBootstrap::init().context(
            "Profile initialization failed. Set SYSTEMPROMPT_PROFILE environment variable to the \
             full path of your profile file",
        )?;

        if requires_secrets {
            SecretsBootstrap::init().context("Secrets initialization failed")?;
        }

        if let Err(e) = CredentialsBootstrap::init() {
            tracing::debug!("Credentials bootstrap: {}", e);
        }

        if requires_secrets {
            let profile = ProfileBootstrap::get()?;
            AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
            Config::try_init().context("Failed to initialize configuration")?;
            FilesConfig::init().context("Failed to initialize files configuration")?;

            let mut validator = StartupValidator::new();
            let report = validator.validate(Config::get()?);

            if report.has_errors() {
                display_validation_report(&report);
                #[allow(clippy::exit)]
                std::process::exit(1);
            }

            if report.has_warnings() {
                display_validation_warnings(&report);
            }
        }

        validate_cloud_credentials();
    }

    match cli.command {
        Some(Commands::Services(cmd)) => services::execute(cmd, &cli_config).await?,
        Some(Commands::Db(cmd)) => db::execute(cmd, &cli_config).await?,
        Some(Commands::Jobs(cmd)) => jobs::execute(cmd, &cli_config).await?,
        Some(Commands::Cloud(cmd)) => cloud::execute(cmd, &cli_config).await?,
        Some(Commands::Agents(cmd)) => agents::execute(cmd).await?,
        Some(Commands::Mcp(cmd)) => mcp::execute(cmd).await?,
        Some(Commands::Logs(cmd)) => logs::execute(cmd, &cli_config).await?,
        Some(Commands::Build(cmd)) => {
            build::execute(cmd, &cli_config)?;
        },
        Some(Commands::Skills(cmd)) => skills::execute(cmd).await?,
        Some(Commands::Users(cmd)) => users::execute(cmd, &cli_config).await?,
        Some(Commands::Files(cmd)) => files::execute(cmd, &cli_config).await?,
        Some(Commands::Setup(args)) => setup::execute(args, &cli_config).await?,
        None => tui::execute(&cli_config).await?,
    }

    Ok(())
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

    cfg
}

fn validate_cloud_credentials() {
    let Ok(profile) = ProfileBootstrap::get() else {
        return;
    };

    let Some(cloud_config) = &profile.cloud else {
        return;
    };

    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            if creds.is_token_expired() {
                CliService::warning(
                    "Cloud token has expired. Run 'systemprompt cloud login' to refresh.",
                );
            }
            if cloud_config.tenant_id.is_none() {
                CliService::warning(
                    "No cloud tenant configured. Run 'systemprompt cloud config' to configure.",
                );
            }
        },
        Ok(None) => {
            if cloud_config.validation != CloudValidationMode::Strict {
                CliService::info("Cloud credentials not configured. Cloud features are disabled.");
            }
        },
        Err(_) => {},
    }
}
