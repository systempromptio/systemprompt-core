//! List MCP package names for build scripts

use anyhow::{Context, Result};
use clap::Args;

use super::types::McpPackagesOutput;
use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListPackagesArgs {
    #[arg(long, help = "Output as space-separated string")]
    pub raw: bool,
}

pub(super) async fn execute(
    args: ListPackagesArgs,
    _config: &CliConfig,
) -> Result<CommandResult<McpPackagesOutput>> {
    let ctx = AppContext::new()
        .await
        .context("Failed to bootstrap AppContext")?;
    let servers = ctx
        .mcp_registry()
        .get_enabled_servers()
        .context("Failed to get enabled MCP servers")?;

    let packages: Vec<String> = servers.iter().map(|s| s.name.clone()).collect();

    let output = if args.raw {
        McpPackagesOutput {
            raw_packages: Some(packages.join(" ")),
            packages,
        }
    } else {
        McpPackagesOutput {
            packages,
            raw_packages: None,
        }
    };

    if args.raw {
        Ok(CommandResult::copy_paste(output).with_title("MCP Packages (raw)"))
    } else {
        Ok(CommandResult::list(output).with_title("MCP Packages"))
    }
}
