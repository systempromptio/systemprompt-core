pub mod cli_settings;
mod commands;
mod presentation;
mod routing;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{
    agents, analytics, build, cloud, config, content, contexts, db, ext, extensions, files, jobs,
    logs, mcp, services, setup, skills, system, users, web,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use systemprompt_cloud::{CliSession, CredentialsBootstrap, ProjectContext};
use systemprompt_core_files::FilesConfig;
use systemprompt_core_logging::{set_startup_mode, CliService};
use systemprompt_models::{AppPaths, Config, ProfileBootstrap, SecretsBootstrap};
use systemprompt_runtime::{
    display_validation_report, display_validation_warnings, DatabaseContext, StartupValidator,
};

use crate::shared::resolve_profile_path;

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
      --database-url    Direct database URL (bypasses profile)")]
struct Cli {
    #[command(flatten)]
    verbosity: VerbosityOpts,

    #[command(flatten)]
    output: OutputOpts,

    #[command(flatten)]
    display: DisplayOpts,

    #[command(flatten)]
    database: DatabaseOpts,

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

    #[command(subcommand, about = "Context management")]
    Contexts(contexts::ContextsCommands),

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

    #[command(subcommand, about = "Content management and analytics")]
    Content(content::ContentCommands),

    #[command(subcommand, about = "Configuration management and rate limits")]
    Config(config::ConfigCommands),

    #[command(subcommand, about = "Analytics and metrics reporting")]
    Analytics(analytics::AnalyticsCommands),

    #[command(subcommand, about = "Web service configuration management")]
    Web(web::WebCommands),

    #[command(subcommand, about = "Extension management and discovery")]
    Extensions(extensions::ExtensionsCommands),

    #[command(subcommand, about = "Run CLI extension commands")]
    Ext(ext::ExtCommands),

    #[command(subcommand, about = "System authentication and session management")]
    System(system::SystemCommands),

    #[command(subcommand, about = "Manage CLI session and profile switching")]
    Session(commands::session::SessionCommands),

    #[command(about = "Interactive setup wizard for local development environment")]
    Setup(setup::SetupArgs),
}

const fn should_skip_validation(command: Option<&Commands>) -> bool {
    matches!(
        command,
        Some(Commands::Skills(skills::SkillsCommands::Create(_)))
    )
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    set_startup_mode(cli.command.is_none());

    let cli_config = build_cli_config(&cli);
    cli_settings::set_global_config(cli_config);

    if cli.display.no_color || !cli_config.should_use_color() {
        console::set_colors_enabled(false);
    }

    if let Some(database_url) = cli.database.database_url.clone() {
        return run_with_database_url(cli.command, &cli_config, &database_url).await;
    }

    let (requires_profile, requires_secrets, requires_paths) = match &cli.command {
        Some(Commands::Cloud(cmd)) => (
            cmd.requires_profile(),
            cmd.requires_secrets(),
            cmd.requires_secrets(),
        ),
        Some(Commands::Setup(_) | Commands::Session(_)) => (false, false, false),
        Some(Commands::Build(_) | Commands::Extensions(_)) => (true, false, false),
        Some(Commands::System(_)) => (true, true, false), /* needs secrets for jwt_secret, but
                                                            * not paths */
        Some(_) | None => (true, true, true),
    };

    if requires_profile {
        let project_ctx = ProjectContext::discover();
        let session_path = project_ctx.local_session();
        let session_profile_path = CliSession::try_load_profile_path(&session_path);

        let profile_path = resolve_profile_path(session_profile_path).context(
            "Profile resolution failed. Set SYSTEMPROMPT_PROFILE environment variable or create a \
             profile with 'systemprompt cloud profile create'",
        )?;

        ProfileBootstrap::init_from_path(&profile_path).with_context(|| {
            format!(
                "Profile initialization failed from: {}",
                profile_path.display()
            )
        })?;

        CredentialsBootstrap::init()
            .await
            .context("Cloud credentials required. Run 'systemprompt cloud login'")?;

        if should_check_remote_routing(cli.command.as_ref()) {
            if let Ok(routing::ExecutionTarget::Remote { hostname, token }) =
                routing::determine_execution_target()
            {
                let args = reconstruct_args(&cli);
                let exit_code =
                    routing::remote::execute_remote(&hostname, &token, &args, 300).await?;
                #[allow(clippy::exit)]
                std::process::exit(exit_code);
            }
        }

        if requires_secrets {
            SecretsBootstrap::init().context("Secrets initialization failed")?;
        }

        if requires_paths {
            let profile = ProfileBootstrap::get()?;
            AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
            Config::try_init().context("Failed to initialize configuration")?;
            FilesConfig::init().context("Failed to initialize files configuration")?;

            if !should_skip_validation(cli.command.as_ref()) {
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
        }

        validate_cloud_credentials();
    }

    match cli.command {
        Some(Commands::Services(cmd)) => services::execute(cmd, &cli_config).await?,
        Some(Commands::Db(cmd)) => db::execute(cmd, &cli_config).await?,
        Some(Commands::Jobs(cmd)) => jobs::execute(cmd, &cli_config).await?,
        Some(Commands::Cloud(cmd)) => cloud::execute(cmd, &cli_config).await?,
        Some(Commands::Agents(cmd)) => agents::execute(cmd).await?,
        Some(Commands::Contexts(cmd)) => contexts::execute(cmd, &cli_config).await?,
        Some(Commands::Mcp(cmd)) => mcp::execute(cmd).await?,
        Some(Commands::Logs(cmd)) => logs::execute(cmd, &cli_config).await?,
        Some(Commands::Build(cmd)) => {
            build::execute(cmd, &cli_config)?;
        },
        Some(Commands::Skills(cmd)) => skills::execute(cmd).await?,
        Some(Commands::Users(cmd)) => users::execute(cmd, &cli_config).await?,
        Some(Commands::Files(cmd)) => files::execute(cmd, &cli_config).await?,
        Some(Commands::Content(cmd)) => content::execute(cmd).await?,
        Some(Commands::Config(cmd)) => config::execute(cmd, &cli_config)?,
        Some(Commands::Analytics(cmd)) => analytics::execute(cmd, &cli_config).await?,
        Some(Commands::Web(cmd)) => web::execute(cmd)?,
        Some(Commands::Extensions(cmd)) => extensions::execute(cmd, &cli_config)?,
        Some(Commands::Ext(cmd)) => ext::execute(cmd, &cli_config).await?,
        Some(Commands::System(cmd)) => system::execute(cmd).await?,
        Some(Commands::Session(cmd)) => commands::session::execute(cmd, &cli_config)?,
        Some(Commands::Setup(args)) => {
            let result = setup::execute(args, &cli_config).await?;
            shared::render_result(&result);
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

    match command {
        Some(Commands::Db(cmd)) => db::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Users(cmd)) => users::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Analytics(cmd)) => analytics::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Content(cmd)) => content::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Logs(cmd)) => logs::execute_with_db(cmd, &db_ctx, config).await,
        Some(Commands::Files(cmd)) => files::execute_with_db(cmd, &db_ctx, config).await,
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

    cfg
}

fn validate_cloud_credentials() {
    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            if creds.is_token_expired() {
                CliService::warning(
                    "Cloud token has expired. Run 'systemprompt cloud login' to refresh.",
                );
            }
        },
        Ok(None) => {
            CliService::error(
                "Cloud credentials not found. Run 'systemprompt cloud login' to register.",
            );
        },
        Err(e) => {
            CliService::error(&format!("Cloud credential error: {}", e));
        },
    }
}

const fn should_check_remote_routing(command: Option<&Commands>) -> bool {
    match command {
        Some(
            Commands::Session(_)
            | Commands::Setup(_)
            | Commands::Cloud(_)
            | Commands::Build(_)
            | Commands::System(_),
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
            )
        {
            args.push(arg.clone());
        }
    }

    args
}
