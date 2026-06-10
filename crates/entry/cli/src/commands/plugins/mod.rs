//! `plugins` command group: discover and inspect compiled extensions.
//!
//! Covers listing and showing extensions, running CLI extension commands,
//! validating dependencies and configuration, dumping config, summarising
//! [`PluginsCommands::Capabilities`], and the [`mcp`] server-management
//! subtree. [`types`] holds the shared serialisable output shapes.

pub mod types;

mod capabilities;
mod config;
mod list;
mod run;
mod show;
mod validate;

pub mod mcp;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::ExtensionRegistry;

use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::shared::render_result;

fn discover_registry() -> ExtensionRegistry {
    ExtensionRegistry::discover().unwrap_or_else(|e| {
        tracing::error!(error = %e, "extension dependency cycle; using empty registry");
        ExtensionRegistry::new()
    })
}

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List all discovered extensions")]
    List(list::ListArgs),

    #[command(about = "Show detailed extension information")]
    Show(show::ShowArgs),

    #[command(about = "Run a CLI extension command", trailing_var_arg = true)]
    Run(run::RunArgs),

    #[command(about = "Validate extension dependencies and configurations")]
    Validate(validate::ValidateArgs),

    #[command(about = "Show extension configuration")]
    Config(config::ConfigArgs),

    #[command(about = "List capabilities across all extensions")]
    Capabilities(capabilities::CapabilitiesArgs),

    #[command(subcommand, about = "MCP server management")]
    Mcp(mcp::McpCommands),
}

impl DescribeCommand for PluginsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Run(_) => CommandDescriptor::PROFILE_ONLY,
            Self::Mcp(_) => CommandDescriptor::FULL,
            _ => CommandDescriptor::PROFILE_ONLY.with_remote_eligible(),
        }
    }
}

pub async fn execute(cmd: PluginsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        PluginsCommands::List(args) => {
            render_result(&list::execute(&args, &ctx.cli), &ctx.cli);
            Ok(())
        },
        PluginsCommands::Show(args) => {
            let result = show::execute(&args, &ctx.cli).context("Failed to show extension")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Run(args) => run::execute(args, ctx).await,
        PluginsCommands::Validate(args) => {
            render_result(&validate::execute(&args, &ctx.cli), &ctx.cli);
            Ok(())
        },
        PluginsCommands::Config(args) => {
            let result =
                config::execute(&args, &ctx.cli).context("Failed to get extension config")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Capabilities(args) => {
            capabilities::execute(args, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Mcp(cmd) => mcp::execute(cmd, ctx).await,
    }
}
