mod list;
mod list_packages;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Subcommand)]
pub enum McpCommands {
    #[command(about = "List MCP server configs")]
    List(list::ListArgs),

    #[command(about = "Validate MCP connection")]
    Validate(validate::ValidateArgs),

    #[command(about = "List package names for build")]
    ListPackages(list_packages::ListPackagesArgs),
}

pub async fn execute(command: McpCommands, config: &CliConfig) -> Result<()> {
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
