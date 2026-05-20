//! MCP server management commands

mod call;
mod call_client;
mod list;
mod list_packages;
mod logs;
mod logs_db;
mod logs_disk;
mod status;
mod tools;
mod tools_client;
mod tools_schema;
pub mod types;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::cli_settings::get_global_config;
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

pub async fn execute(command: McpCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: McpCommands, config: &CliConfig) -> Result<()> {
    match command {
        McpCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list MCP servers")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get MCP server status")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::Validate(args) => {
            let result = validate::execute(args, config)
                .await
                .context("Failed to validate MCP server")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::Logs(args) => {
            let result = logs::execute(args, config)
                .await
                .context("Failed to get MCP server logs")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::ListPackages(args) => {
            let result =
                list_packages::execute(args, config).context("Failed to list MCP packages")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::Tools(args) => {
            let result = tools::execute(args, config)
                .await
                .context("Failed to list MCP tools")?;
            render_result(&result);
            Ok(())
        },
        McpCommands::Call(args) => {
            let result = call::execute(args, config)
                .await
                .context("Failed to execute MCP tool")?;
            render_result(&result);
            Ok(())
        },
    }
}
