mod list;
mod show;
mod stats;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use super::types::{MessageRow, ToolCallRow};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RequestCommands {
    #[command(
        about = "List recent AI requests",
        after_help = "EXAMPLES:\n  systemprompt logs request list\n  systemprompt logs request \
                      list --model gpt-4 --since 1h"
    )]
    List(list::ListArgs),

    #[command(
        about = "Show AI request details",
        after_help = "EXAMPLES:\n  systemprompt logs request show abc123\n  systemprompt logs \
                      request show abc123 --messages --tools"
    )]
    Show(show::ShowArgs),

    #[command(
        about = "Show aggregate AI request statistics",
        after_help = "EXAMPLES:\n  systemprompt logs request stats\n  systemprompt logs request \
                      stats --since 24h"
    )]
    Stats(stats::StatsArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestListRow {
    pub request_id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub tokens: String,
    pub cost: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestListOutput {
    pub requests: Vec<RequestListRow>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestShowOutput {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub messages: Vec<MessageRow>,
    pub linked_mcp_calls: Vec<ToolCallRow>,
}

pub async fn execute(command: RequestCommands, config: &CliConfig) -> Result<()> {
    match command {
        RequestCommands::List(args) => list::execute(args, config).await,
        RequestCommands::Show(args) => show::execute(args, config).await,
        RequestCommands::Stats(args) => stats::execute(args, config).await,
    }
}

pub async fn execute_with_pool(
    command: RequestCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        RequestCommands::List(args) => list::execute_with_pool(args, db_ctx, config).await,
        RequestCommands::Show(args) => show::execute_with_pool(args, db_ctx, config).await,
        RequestCommands::Stats(args) => stats::execute_with_pool(args, db_ctx, config).await,
    }
}
