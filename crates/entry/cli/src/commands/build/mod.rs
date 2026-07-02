//! The `build` command group for compiling the workspace and MCP extensions.
//!
//! [`BuildCommands`] dispatches to the core workspace build and the MCP
//! extension build; [`types`] holds the shared result rows surfaced by both.

mod core;
pub mod mcp;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum BuildCommands {
    #[command(about = "Build Rust workspace (systemprompt-core)")]
    Core(core::CoreArgs),

    #[command(about = "Build MCP extensions")]
    Mcp(mcp::McpArgs),
}

pub fn execute(cmd: BuildCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        BuildCommands::Core(args) => {
            let result = core::execute(args, &ctx.cli).context("Failed to build core")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BuildCommands::Mcp(args) => {
            let result = mcp::execute(args, &ctx.cli).context("Failed to build MCP extensions")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
