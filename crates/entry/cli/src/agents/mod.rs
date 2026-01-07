#[allow(clippy::module_inception)]
pub mod agents;
pub mod mcp;

use anyhow::{Context, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

#[derive(Subcommand)]
pub enum AgentsCommands {
    #[command(subcommand, about = "A2A agent management")]
    Agent(agents::AgentCommands),

    #[command(subcommand, about = "MCP server management")]
    Mcp(mcp::McpCommands),
}

pub async fn execute(command: AgentsCommands) -> Result<()> {
    match command {
        AgentsCommands::Agent(cmd) => {
            let ctx = Arc::new(
                AppContext::new()
                    .await
                    .context("Failed to initialize application context")?,
            );
            agents::execute(cmd, ctx).await
        },
        AgentsCommands::Mcp(cmd) => mcp::execute(cmd).await,
    }
}
