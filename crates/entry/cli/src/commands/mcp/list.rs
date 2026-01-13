//! List MCP server configs

use anyhow::{Context, Result};
use clap::Args;

use super::types::{McpListOutput, McpServerSummary};
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;

#[derive(Args)]
pub struct ListArgs {
    /// Show only enabled servers
    #[arg(long, help = "Show only enabled servers")]
    pub enabled: bool,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<McpListOutput>> {
    let services_config =
        ConfigLoader::load().context("Failed to load services configuration")?;

    let mut servers: Vec<McpServerSummary> = services_config
        .mcp_servers
        .iter()
        .filter(|(_, server)| {
            if args.enabled {
                server.enabled
            } else {
                true
            }
        })
        .map(|(name, server)| McpServerSummary {
            name: name.clone(),
            port: server.port,
            enabled: server.enabled,
            status: if server.enabled {
                "configured"
            } else {
                "disabled"
            }
            .to_string(),
        })
        .collect();

    servers.sort_by(|a, b| a.name.cmp(&b.name));

    let output = McpListOutput { servers };

    Ok(CommandResult::table(output)
        .with_title("MCP Servers")
        .with_columns(vec![
            "name".to_string(),
            "port".to_string(),
            "enabled".to_string(),
            "status".to_string(),
        ]))
}
