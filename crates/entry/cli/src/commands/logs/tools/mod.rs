mod list;
mod queries;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ToolsCommands {
    #[command(
        about = "List MCP tool executions",
        after_help = "EXAMPLES:\n  systemprompt logs tools list\n  systemprompt logs tools list \
                      --name research_blog\n  systemprompt logs tools list --server \
                      content-manager --since 1h"
    )]
    List(list::ListArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolExecutionRow {
    pub timestamp: String,
    pub trace_id: String,
    pub tool_name: String,
    pub server: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolsListOutput {
    pub executions: Vec<ToolExecutionRow>,
    pub total: u64,
}

pub async fn execute(command: ToolsCommands, config: &CliConfig) -> Result<()> {
    match command {
        ToolsCommands::List(args) => list::execute(args, config).await,
    }
}
