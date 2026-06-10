//! `infra logs tools` subcommands for listing MCP tool executions.
//!
//! Exposes [`ToolsCommands`] and the [`ToolExecutionRow`] / [`ToolsListOutput`]
//! shapes rendered for each execution record.

mod list;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TraceId;

use crate::context::CommandContext;

#[derive(Debug, Subcommand)]
pub enum ToolsCommands {
    #[command(
        about = "List MCP tool executions",
        after_help = "EXAMPLES:\n  systemprompt infra logs tools list\n  systemprompt infra logs \
                      tools list --name research_blog\n  systemprompt infra logs tools list \
                      --server content-manager --since 1h"
    )]
    List(list::ListArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolExecutionRow {
    pub timestamp: String,
    pub trace_id: TraceId,
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

pub async fn execute(command: ToolsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        ToolsCommands::List(args) => list::execute(args, ctx).await,
    }
}
