use clap::{Parser, Subcommand};

use crate::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
use crate::commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};
use crate::descriptor::{CommandDescriptor, DescribeCommand};

#[derive(clap::Args)]
pub struct VerbosityOpts {
    #[arg(
        long,
        short = 'v',
        global = true,
        hide = true,
        help = "Increase verbosity"
    )]
    pub verbose: bool,

    #[arg(
        long,
        short = 'q',
        global = true,
        hide = true,
        conflicts_with = "verbose",
        help = "Suppress output"
    )]
    pub quiet: bool,

    #[arg(long, global = true, hide = true, help = "Debug logging")]
    pub debug: bool,
}

#[derive(clap::Args)]
pub struct OutputOpts {
    #[arg(long, global = true, hide = true, help = "JSON output")]
    pub json: bool,

    #[arg(
        long,
        global = true,
        hide = true,
        conflicts_with = "json",
        help = "YAML output"
    )]
    pub yaml: bool,
}

#[derive(clap::Args)]
pub struct DisplayOpts {
    #[arg(long, global = true, hide = true, help = "Disable colors")]
    pub no_color: bool,

    #[arg(long, global = true, hide = true, help = "Non-interactive mode")]
    pub non_interactive: bool,
}

#[derive(clap::Args)]
pub struct DatabaseOpts {
    #[arg(
        long,
        global = true,
        env = "SYSTEMPROMPT_DATABASE_URL",
        help = "Direct database URL (bypasses profile)"
    )]
    pub database_url: Option<String>,
}

#[derive(clap::Args)]
pub struct ProfileOpts {
    #[arg(
        long,
        global = true,
        help = "Profile name to use (overrides active session)"
    )]
    pub profile: Option<String>,
}

#[derive(Parser)]
#[command(name = "systemprompt")]
#[command(
    about = "Agent orchestration and AI operations. Run 'systemprompt core playbooks list' for \
             workflow guides."
)]
#[command(version = "0.1.0")]
#[command(
    before_help = "\x1b[38;5;208m</\x1b[1;37mSYSTEMPROMPT\x1b[38;5;208m.\x1b[0;37mio\x1b[38;5;\
                   208m>\x1b[0m"
)]
#[command(after_help = "\
GETTING STARTED:
  systemprompt core playbooks list              List all workflow playbooks
  systemprompt core playbooks show info_start   View the getting started guide

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
pub struct Cli {
    #[command(flatten)]
    pub verbosity: VerbosityOpts,

    #[command(flatten)]
    pub output: OutputOpts,

    #[command(flatten)]
    pub display: DisplayOpts,

    #[command(flatten)]
    pub database: DatabaseOpts,

    #[command(flatten)]
    pub profile_opts: ProfileOpts,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(
        subcommand,
        about = "Core operations: playbooks, skills, content, files, contexts"
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

impl DescribeCommand for Commands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Cloud(cmd) => cmd.descriptor(),
            Self::Plugins(cmd) => cmd.descriptor(),
            Self::Admin(admin::AdminCommands::Setup(_)) => CommandDescriptor::NONE,
            Self::Admin(admin::AdminCommands::Session(cmd)) => cmd.descriptor(),
            Self::Build(_) => CommandDescriptor::PROFILE_ONLY,
            Self::Core(core::CoreCommands::Skills(core::skills::SkillsCommands::Create(_))) => {
                CommandDescriptor::FULL.with_skip_validation()
            },
            _ => CommandDescriptor::FULL,
        }
    }
}

pub fn build_cli_config(cli: &Cli) -> CliConfig {
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

pub fn reconstruct_args(cli: &Cli) -> Vec<String> {
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
    let mut skip_next = false;
    for arg in &original_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--profile" {
            skip_next = true;
            continue;
        }
        if arg.starts_with("--profile=") {
            continue;
        }
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
