//! The `build` command group for compiling the workspace and MCP extensions.
//!
//! [`BuildCommands`] dispatches to the core workspace build and the MCP
//! extension build; [`types`] holds the shared result rows surfaced by both.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod core;
pub mod mcp;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
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
    const NAME: &'static str = "build";
    const DISPLAY_ORDER: usize = 80;
    const ABOUT: &'static str = "Build MCP extensions";

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send {
        std::future::ready(execute(self, host).map_err(CommandError::new))
    }
}

pub fn execute(cmd: BuildCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        BuildCommands::Core(args) => {
            core::execute(args, host.output(), host.children()).context("Failed to build core")
        },
        BuildCommands::Mcp(args) => mcp::execute(args, host.output(), host.children())
            .context("Failed to build MCP extensions"),
    }
}
