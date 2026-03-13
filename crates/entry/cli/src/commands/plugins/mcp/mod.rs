//! MCP server management commands

mod call;
mod list;
mod list_packages;
mod logs;
mod status;
mod tools;
pub mod types;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::cli_settings::get_global_config;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    List(list::ListArgs),

    Status(status::StatusArgs),

    Validate(validate::ValidateArgs),

    Logs(logs::LogsArgs),

    ListPackages(list_packages::ListPackagesArgs),

    Tools(tools::ToolsArgs),

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
