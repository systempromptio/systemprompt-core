//! MCP server management commands

mod list;
mod list_packages;
pub mod types;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Subcommand)]
pub enum McpCommands {
    /// List MCP server configs
    List(list::ListArgs),

    /// Validate MCP connection
    Validate(validate::ValidateArgs),

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
            let result = list::execute(args, config)
                .await
                .context("Failed to list MCP servers")?;
            render_result(&result);
            Ok(())
        }
        McpCommands::Validate(args) => {
            let result = validate::execute(args, config)
                .await
                .context("Failed to validate MCP server")?;
            render_result(&result);
            Ok(())
        }
        McpCommands::ListPackages(args) => {
            let result = list_packages::execute(args, config)
                .await
                .context("Failed to list MCP packages")?;
            render_result(&result);
            Ok(())
        }
    }
}
