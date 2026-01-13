//! MCP server management commands

mod list;
mod list_packages;
mod logs;
mod status;
pub mod types;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    /// List MCP server configs
    List(list::ListArgs),

    /// Show running MCP server status with binary info
    Status(status::StatusArgs),

    /// Validate MCP connection
    Validate(validate::ValidateArgs),

    /// View MCP server logs
    Logs(logs::LogsArgs),

    /// List package names for build
    ListPackages(list_packages::ListPackagesArgs),
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
    }
}
