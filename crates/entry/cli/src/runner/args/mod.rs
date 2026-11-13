//! Top-level clap argument definitions and the command tree.
//!
//! Defines the global option groups, the [`Cli`] parser, and the kernel's
//! [`Commands`] subcommand tree, along with the argument-reconstruction used
//! when forwarding to a remote tenant. Each group declares its own bootstrap
//! [`CommandDescriptor`]; [`command`] assembles the full tree from the kernel
//! groups plus every group registered through the [`CommandRegistry`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::{ArgMatches, Command, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::commands::{admin, analytics, cloud, core, infrastructure, plugins, web};
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::kernel::{CommandRegistry, RegistryError};

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
    #[arg(long, global = true, help = "Direct database URL (bypasses profile)")]
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
}

impl DescribeCommand for Commands {
    fn descriptor(&self) -> CommandDescriptor {
        let bootstrap = match self {
            Self::Core(cmd) => cmd.descriptor(),
            Self::Infra(cmd) => cmd.descriptor(),
            Self::Admin(cmd) => cmd.descriptor(),
            Self::Cloud(cmd) => cmd.descriptor(),
            Self::Analytics(cmd) => cmd.descriptor(),
            Self::Web(cmd) => cmd.descriptor(),
            Self::Plugins(cmd) => cmd.descriptor(),
        };
        bootstrap.with_data_impact(self.data_impact())
    }
}

impl Cli {
    pub fn from_globals(matches: &ArgMatches) -> Result<Self, clap::Error> {
        Ok(Self {
            verbosity: VerbosityOpts::from_arg_matches(matches)?,
            output: OutputOpts::from_arg_matches(matches)?,
            display: DisplayOpts::from_arg_matches(matches)?,
            database: DatabaseOpts::from_arg_matches(matches)?,
            profile_opts: ProfileOpts::from_arg_matches(matches)?,
            command: None,
        })
    }
}

pub fn command(registry: &CommandRegistry) -> Result<Command, RegistryError> {
    registry.assemble(Cli::command())
}

mod assemble;
mod impact;

pub use assemble::{
    build_cli_config, has_local_export_flag, has_local_export_flag_in, reconstruct_args,
    reconstruct_args_from,
};
