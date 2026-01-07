mod agents;
mod build;
pub mod cli_settings;
mod cloud;
pub mod common;
mod logs;
mod presentation;
mod services;
mod setup;
mod tui;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};

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
    #[arg(long, short = 'v', global = true)]
    verbose: bool,

    #[arg(long, short = 'q', global = true, conflicts_with = "verbose")]
    quiet: bool,

    #[arg(long, global = true)]
    debug: bool,
}

#[derive(clap::Args)]
struct OutputOpts {
    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true, conflicts_with = "json")]
    yaml: bool,
}

#[derive(clap::Args)]
struct DisplayOpts {
    #[arg(long, global = true)]
    no_color: bool,

    #[arg(long, global = true)]
    non_interactive: bool,
}

#[derive(Parser)]
#[command(name = "systemprompt")]
#[command(
    about = "SystemPrompt OS - Unified CLI for agent orchestration, AI operations, and system \
             management"
)]
#[command(version = "0.1.0")]
#[command(long_about = None)]
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
        about = "Service lifecycle management (start, stop, db, scheduler)"
    )]
    Services(services::ServicesCommands),

    #[command(subcommand, about = "Cloud deployment, sync, and setup")]
    Cloud(cloud::CloudCommands),

    #[command(subcommand, about = "Agent and MCP server management")]
    Agents(agents::AgentsCommands),

    #[command(subcommand, about = "Log streaming and tracing")]
    Logs(logs::LogsCommands),

    #[command(subcommand, about = "Build MCP extensions")]
    Build(build::BuildCommands),

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
        Some(Commands::Setup(_) | Commands::Build(_)) => (false, false),
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
        Some(Commands::Services(cmd)) => services::execute(cmd).await?,
        Some(Commands::Cloud(cmd)) => cloud::execute(cmd).await?,
        Some(Commands::Agents(cmd)) => agents::execute(cmd).await?,
        Some(Commands::Logs(cmd)) => logs::execute(cmd).await?,
        Some(Commands::Build(cmd)) => build::execute(cmd).await?,
        Some(Commands::Setup(args)) => setup::execute(args).await?,
        None => tui::execute().await?,
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
    let profile = match ProfileBootstrap::get() {
        Ok(p) => p,
        Err(_) => return,
    };

    let cloud_config = match &profile.cloud {
        Some(config) if config.cli_enabled => config,
        _ => return,
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
