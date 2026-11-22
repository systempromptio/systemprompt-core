//! `core artifacts`: the caller's agent artifacts.
//!
//! `agent-http` contributes the group under the kernel's `core` group with
//! the `caller-session` grant; every command reads only artifacts the
//! caller's session user owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod list;
pub mod show;
pub mod types;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};

pub use types::{ArtifactListOutput, ArtifactPartOutput, ArtifactSummary};

#[derive(Debug, Subcommand)]
pub enum ArtifactsCommands {
    #[command(about = "List artifacts")]
    List(list::ListArgs),

    #[command(about = "Show artifact details and content")]
    Show(show::ShowArgs),
}

pub async fn execute(cmd: ArtifactsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        ArtifactsCommands::List(args) => Box::pin(list::execute(args, host)).await,
        ArtifactsCommands::Show(args) => Box::pin(show::execute(args, host)).await,
    }
}

impl DescribeCommand for ArtifactsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandGroup for ArtifactsCommands {
    const NAME: &'static str = "artifacts";
    const ABOUT: &'static str = "Artifact inspection and debugging";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(crate::cli::command_error)
    }
}
