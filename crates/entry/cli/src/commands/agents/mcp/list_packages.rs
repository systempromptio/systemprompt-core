use anyhow::{Context, Result};
use clap::Args;

use crate::shared::CommandResult;
use crate::CliConfig;
use super::super::types::McpPackagesOutput;
use systemprompt_core_mcp::services::registry::RegistryManager;

#[derive(Args)]
pub struct ListPackagesArgs {
    #[arg(long, help = "Output as space-separated string")]
    pub raw: bool,
}

pub async fn execute(
    args: ListPackagesArgs,
    _config: &CliConfig,
) -> Result<CommandResult<McpPackagesOutput>> {
    let servers = RegistryManager::get_enabled_servers()
        .context("Failed to get enabled MCP servers")?;

    let packages: Vec<String> = servers.iter().map(|s| s.name.clone()).collect();

    let output = McpPackagesOutput { packages };

    if args.raw {
        Ok(CommandResult::copy_paste(output).with_title("MCP Packages"))
    } else {
        Ok(CommandResult::list(output).with_title("MCP Packages"))
    }
}
