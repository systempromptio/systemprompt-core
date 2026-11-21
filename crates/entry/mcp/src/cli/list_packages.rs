//! `plugins mcp list-packages`: the enabled MCP servers' package names, read
//! from the registry the `mcp` member provides, for build scripts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_models::artifacts::ListItem;

use super::registry;
use super::types::McpPackagesOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListPackagesArgs {
    #[arg(long, help = "Output as space-separated string")]
    pub raw: bool,
}

pub(super) async fn execute(
    args: ListPackagesArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    let servers = registry(host)
        .await?
        .list_enabled_servers()
        .await
        .context("Failed to get enabled MCP servers")?;

    let packages: Vec<String> = servers.into_iter().map(|s| s.name).collect();

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
