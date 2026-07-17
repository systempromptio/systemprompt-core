//! MCP server management commands
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod call;
mod call_client;
mod list;
mod list_packages;
pub mod logs;
pub mod logs_db;
mod logs_disk;
mod status;
mod tools;
mod tools_client;
mod tools_schema;
pub mod types;
pub mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

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

pub async fn execute(command: McpCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match command {
        McpCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list MCP servers")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get MCP server status")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::Validate(args) => {
            let result = validate::execute(args, ctx.prompter(), config)
                .await
                .context("Failed to validate MCP server")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::Logs(args) => {
            let result = logs::execute(args, ctx.prompter(), config)
                .await
                .context("Failed to get MCP server logs")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::ListPackages(args) => {
            let result = list_packages::execute(args, config)
                .await
                .context("Failed to list MCP packages")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::Tools(args) => {
            let result = tools::execute(args, ctx)
                .await
                .context("Failed to list MCP tools")?;
            render_result(&result, config);
            Ok(())
        },
        McpCommands::Call(args) => {
            let result = call::execute(args, ctx)
                .await
                .context("Failed to execute MCP tool")?;
            render_result(&result, config);
            Ok(())
        },
    }
}
