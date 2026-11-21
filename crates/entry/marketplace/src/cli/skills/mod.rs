//! `core skills` command group: list and show configured skills.
//!
//! [`SkillsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! against the command host. Disk skills are read from the services tree
//! through the services-tree grant; managed skills through the owner-bound
//! organization skill resolver the `managed_resources` member provides.
//! Output payload types live in [`types`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod list;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};

#[derive(Debug, Subcommand)]
pub enum SkillsCommands {
    #[command(about = "List configured skills")]
    List(list::ListArgs),

    #[command(about = "Show skill details")]
    Show(show::ShowArgs),
}

pub async fn execute(command: SkillsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        SkillsCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list skills"),
        SkillsCommands::Show(args) => list::show_resolved_skill(&args.name, host)
            .await
            .context("Failed to show skill"),
    }
}

impl SkillsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_) | Self::Show(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for SkillsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for SkillsCommands {
    const NAME: &'static str = "skills";
    const ABOUT: &'static str = "Skill management and database sync";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(super::command_error)
    }
}
