//! `plugins` CLI command group: list, show, validate, and generate plugins.
//!
//! [`PluginsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! to its submodule and renders the result. Output payload types live in
//! [`types`].

pub mod types;

mod generate;
mod list;
mod show;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List configured plugins")]
    List(list::ListArgs),

    #[command(about = "Show plugin details")]
    Show(show::ShowArgs),

    #[command(about = "Validate plugin configuration")]
    Validate(validate::ValidateArgs),

    #[command(about = "Generate Claude Code plugin output")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: PluginsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        PluginsCommands::List(args) => {
            let result = list::execute(args, &ctx.cli).context("Failed to list plugins")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Show(args) => {
            let result = show::execute(&args, &ctx.cli).context("Failed to show plugin")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Validate(args) => {
            let result = validate::execute(args, &ctx.cli).context("Failed to validate plugins")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Generate(args) => {
            let result =
                generate::execute(&args, &ctx.cli).context("Failed to generate plugins")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
