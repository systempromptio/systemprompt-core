//! MCP server management commands
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod call;
pub mod call_client;
pub mod list;
mod list_packages;
pub mod logs;
pub mod logs_db;
mod logs_disk;
mod status;
mod tools;
mod tools_schema;
pub mod types;
pub mod validate;
mod validate_output;

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::CommandOutput;


#[derive(Debug, Subcommand)]
pub enum McpCommands {
    #[command(about = "List configured MCP servers")]
    List(list::ListArgs),

    #[command(about = "Show MCP server runtime status")]
    Status(status::StatusArgs),

    #[command(about = "Validate MCP server configurations")]
    Validate(validate::ValidateArgs),

    #[command(about = "Tail logs for an MCP server")]
    Logs(logs::LogsArgs),

    #[command(about = "List discovered MCP packages from the registry")]
    ListPackages(list_packages::ListPackagesArgs),

    #[command(about = "List tools exposed by enabled MCP servers")]
    Tools(tools::ToolsArgs),

    #[command(about = "Invoke a tool on an MCP server")]
    Call(call::CallArgs),
}

pub async fn execute(command: McpCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        McpCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list MCP servers"),
        McpCommands::Status(args) => status::execute(args, host)
            .await
            .context("Failed to get MCP server status"),
        McpCommands::Validate(args) => {
            let (result, valid) = Box::pin(validate::execute(args, host))
                .await
                .context("Failed to validate MCP server")?;
            if valid {
                return Ok(result);
            }
            host.render(&result);
            anyhow::bail!("MCP server validation failed")
        },
        McpCommands::Logs(args) => logs::execute(args, host)
            .await
            .context("Failed to get MCP server logs"),
        McpCommands::ListPackages(args) => list_packages::execute(args, host)
            .await
            .context("Failed to list MCP packages"),
        McpCommands::Tools(args) => Box::pin(tools::execute(args, host))
            .await
            .context("Failed to list MCP tools"),
        McpCommands::Call(args) => Box::pin(call::execute(args, host))
            .await
            .context("Failed to execute MCP tool"),
    }
}
