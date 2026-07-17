//! List MCP package names for build scripts
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;

use super::types::McpPackagesOutput;
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_models::artifacts::ListItem;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListPackagesArgs {
    #[arg(long, help = "Output as space-separated string")]
    pub raw: bool,
}

pub(super) async fn execute(args: ListPackagesArgs, _config: &CliConfig) -> Result<CommandOutput> {
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
        Ok(CommandOutput::copy_paste_titled(
            "MCP Packages (raw)",
            output.raw_packages.unwrap_or_default(),
        ))
    } else {
        let items: Vec<ListItem> = output
            .packages
            .iter()
            .map(|name| ListItem::new(name.clone(), String::new(), name.clone()))
            .collect();
        Ok(CommandOutput::list(items).with_title("MCP Packages"))
    }
}
