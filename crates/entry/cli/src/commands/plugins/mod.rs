//! `plugins` command group: discover and inspect extensions.
//!
//! Covers listing and showing the members the composition admitted (and the
//! manifest-discovered extensions beside them), running CLI extension
//! commands, dumping configuration sections, summarising
//! [`PluginsCommands::Capabilities`], listing the admitted composition member
//! by member ([`seams`]). The `plugins mcp` subtree is contributed by the
//! `mcp-http` member. [`types`] holds the shared serialisable output shapes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod capabilities;
mod composition;
pub mod config;
pub mod list;
mod run;
pub mod seams;
pub mod show;

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{CommandDescriptor, CommandOutput, DataImpact, DescribeCommand};

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List all discovered extensions")]
    List(list::ListArgs),

    #[command(about = "Show detailed extension information")]
    Show(show::ShowArgs),

    #[command(about = "Run a CLI extension command", trailing_var_arg = true)]
    Run(run::RunArgs),

    #[command(about = "Show extension configuration")]
    Config(config::ConfigArgs),

    #[command(about = "List capabilities across all extensions")]
    Capabilities(capabilities::CapabilitiesArgs),

    #[command(about = "List the admitted composition, member by member")]
    Seams(seams::SeamsArgs),
}

impl DescribeCommand for PluginsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Run(_) => CommandDescriptor::PROFILE_AND_SECRETS,
            _ => CommandDescriptor::PROFILE_ONLY.with_remote_eligible(),
        }
    }
}

pub async fn execute(cmd: PluginsCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        PluginsCommands::List(args) => Ok(list::execute(&args, host)?),
        PluginsCommands::Show(args) => {
            show::execute(&args, host).context("Failed to show extension")
        },
        PluginsCommands::Run(args) => {
            run::execute(args, host).await?;
            Ok(CommandOutput::rendered())
        },
        PluginsCommands::Config(args) => {
            config::execute(&args, host).context("Failed to get extension config")
        },
        PluginsCommands::Capabilities(args) => Ok(capabilities::execute(args, host)?),
        PluginsCommands::Seams(args) => Ok(seams::execute(&args, host)?),
    }
}

impl PluginsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_)
            | Self::Show(_)
            | Self::Run(_)
            | Self::Config(_)
            | Self::Capabilities(_)
            | Self::Seams(_) => DataImpact::Preserving,
        }
    }
}
