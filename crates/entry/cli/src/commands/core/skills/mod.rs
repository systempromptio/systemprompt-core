//! `skills` CLI command group: list and show configured skills.
//!
//! [`SkillsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! against the command host. Output payload types live in [`types`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod list;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::{CommandHost, CommandOutput};

#[derive(Debug, Subcommand)]
pub enum SkillsCommands {
    #[command(about = "List configured skills")]
    List(list::ListArgs),

    #[command(about = "Show skill details")]
    Show(show::ShowArgs),
}

pub async fn execute(command: SkillsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        SkillsCommands::List(args) => {
            list::execute(args, host)
                .await
                .context("Failed to list skills")
        },
        SkillsCommands::Show(args) => {
            list::show_resolved_skill(&args.name, host)
                .await
                .context("Failed to show skill")
        },
    }
}
