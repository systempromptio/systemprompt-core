mod core;
mod mcp;
pub mod types;
mod web;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum BuildCommands {
    #[command(about = "Build Rust workspace (systemprompt-core)")]
    Core(core::CoreArgs),

    #[command(about = "Build web frontend")]
    Web(web::WebArgs),

    #[command(about = "Build MCP extensions")]
    Mcp(mcp::McpArgs),
}

pub fn execute(cmd: BuildCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        BuildCommands::Core(args) => {
            let result = core::execute(args, config).context("Failed to build core")?;
            render_result(&result);
            Ok(())
        },
        BuildCommands::Web(args) => {
            let result = web::execute(args, config).context("Failed to build web")?;
            render_result(&result);
            Ok(())
        },
        BuildCommands::Mcp(args) => {
            let result = mcp::execute(args, config).context("Failed to build MCP extensions")?;
            render_result(&result);
            Ok(())
        },
    }
}
