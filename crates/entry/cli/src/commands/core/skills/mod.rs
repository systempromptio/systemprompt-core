//! `skills` CLI command group: list and show configured skills.
//!
//! [`SkillsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! against the invocation's [`CommandContext`]. Output payload types live in
//! [`types`].

pub mod types;

mod list;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SkillsCommands {
    #[command(about = "List configured skills")]
    List(list::ListArgs),

    #[command(about = "Show skill details")]
    Show(show::ShowArgs),
}

pub fn execute(command: SkillsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        SkillsCommands::List(args) => {
            let result = list::execute(args, &ctx.cli).context("Failed to list skills")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SkillsCommands::Show(args) => {
            let result = show::execute(&args, &ctx.cli).context("Failed to show skill")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
