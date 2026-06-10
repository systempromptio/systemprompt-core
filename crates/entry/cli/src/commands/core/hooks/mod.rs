//! `core hooks` command group: list and validate plugin hook definitions.
//!
//! Dispatches the [`HooksCommands`] subcommands (list, validate) that enumerate
//! hooks across installed plugins and check their definitions.

pub mod types;

mod list;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum HooksCommands {
    #[command(about = "List hooks across all plugins")]
    List(list::ListArgs),

    #[command(about = "Validate all hook definitions")]
    Validate(validate::ValidateArgs),
}

pub fn execute(command: HooksCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        HooksCommands::List(args) => {
            let result = list::execute(args, &ctx.cli).context("Failed to list hooks")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        HooksCommands::Validate(args) => {
            let result = validate::execute(args, &ctx.cli).context("Failed to validate hooks")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
