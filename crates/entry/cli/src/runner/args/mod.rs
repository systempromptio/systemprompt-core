//! Top-level clap argument definitions and the command tree.
//!
//! Defines the global option groups, the [`Cli`] parser, and the [`Commands`]
//! subcommand tree, along with the mapping from each command to its bootstrap
//! [`CommandDescriptor`] and the argument-reconstruction used when forwarding
//! to a remote tenant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::{Parser, Subcommand};

use crate::commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};
use crate::descriptor::{CommandDescriptor, DescribeCommand};

#[derive(Debug, Clone, Copy, clap::Args)]
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

#[derive(Debug, Clone, Copy, clap::Args)]
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

#[derive(Debug, Clone, Copy, clap::Args)]
pub struct DisplayOpts {
    #[arg(long, global = true, hide = true, help = "Disable colors")]
    pub no_color: bool,

    #[arg(long, global = true, hide = true, help = "Non-interactive mode")]
    pub non_interactive: bool,
}

#[derive(Debug, clap::Args)]
pub struct DatabaseOpts {
    #[arg(
        long,
        global = true,
        env = "SYSTEMPROMPT_DATABASE_URL",
        help = "Direct database URL (bypasses profile)"
    )]
    pub database_url: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ProfileOpts {
    #[arg(
        long,
        global = true,
        help = "Profile name to use (overrides active session)"
    )]
    pub profile: Option<String>,
}

#[derive(Debug, Parser)]
#[command(name = "systemprompt")]
#[command(about = "Agent orchestration and AI operations.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(
    before_help = "\x1b[38;5;208m</\x1b[1;37mSYSTEMPROMPT\x1b[38;5;208m.\x1b[0;37mio\x1b[38;5;\
                   208m>\x1b[0m"
)]
#[command(after_help = "\
GETTING STARTED:
  systemprompt core skills list                 List all skills

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

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        subcommand,
        about = "Core operations: skills, content, files, contexts"
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

    #[command(subcommand, about = "Cloud deployment, backup, and setup")]
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
            Self::Admin(admin::AdminCommands::Config(admin::config::ConfigCommands::Secret(_)))
            | Self::Build(_) => CommandDescriptor::PROFILE_ONLY,
            Self::Admin(admin::AdminCommands::Config(_))
            | Self::Web(_)
            | Self::Core(
                core::CoreCommands::Hooks(_)
                | core::CoreCommands::Plugins(_)
                | core::CoreCommands::Skills(
                    core::skills::SkillsCommands::List(_) | core::skills::SkillsCommands::Show(_),
                ),
            ) => CommandDescriptor::PROFILE_SECRETS_AND_PATHS,
            Self::Infra(infrastructure::InfraCommands::Services(_)) => {
                CommandDescriptor::PROFILE_SECRETS_AND_PATHS
            },
            Self::Infra(infrastructure::InfraCommands::Jobs(
                infrastructure::jobs::JobsCommands::Run(_)
                | infrastructure::jobs::JobsCommands::List,
            )) => CommandDescriptor::FULL.with_skip_validation(),
            // Reads. They may fall back to local data with a warning rather
            // than refusing when a cloud profile cannot route remotely.
            Self::Analytics(_) => CommandDescriptor::FULL
                .with_skip_validation()
                .with_read_only(),
            Self::Infra(infrastructure::InfraCommands::Logs(_)) => {
                CommandDescriptor::FULL.with_read_only()
            },
            _ => CommandDescriptor::FULL,
        }
    }
}

mod assemble;

pub use assemble::{
    build_cli_config, has_local_export_flag, has_local_export_flag_in, reconstruct_args,
    reconstruct_args_from,
};
