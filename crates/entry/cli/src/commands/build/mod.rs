//! The `build` command group for compiling the workspace and MCP extensions.
//!
//! [`BuildCommands`] dispatches to the core workspace build and the MCP
//! extension build; [`types`] holds the shared result rows surfaced by both.
//! The group registers itself with the kernel through
//! [`register_cli_command!`](crate::register_cli_command) rather than being
//! named in the kernel's command enum.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod core;
pub mod mcp;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::kernel::{
    CliCommandGroup, CommandContext, CommandDescriptor, DataImpact, DescribeCommand, render_result,
};

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum BuildCommands {
    #[command(about = "Build Rust workspace (systemprompt-core)")]
    Core(core::CoreArgs),

    #[command(about = "Build MCP extensions")]
    Mcp(mcp::McpArgs),
}

impl DescribeCommand for BuildCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_ONLY.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandGroup for BuildCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "build";
    const ABOUT: &'static str = "Build MCP extensions";

    fn run(self, ctx: &CommandContext) -> impl Future<Output = Result<()>> {
        std::future::ready(execute(self, ctx))
    }
}

crate::register_cli_command!(BuildCommands);

pub fn execute(cmd: BuildCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        BuildCommands::Core(args) => {
            let result = core::execute(args, &ctx.cli, ctx.foreground_children())
                .context("Failed to build core")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BuildCommands::Mcp(args) => {
            let result = mcp::execute(args, &ctx.cli, ctx.foreground_children())
                .context("Failed to build MCP extensions")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
