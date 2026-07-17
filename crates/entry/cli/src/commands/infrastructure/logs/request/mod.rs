//! `infra logs request` subcommands for inspecting AI provider requests.
//!
//! Exposes [`RequestCommands`] (list, show, stats) and the row types
//! ([`RequestListRow`], [`RequestShowOutput`]) returned to the renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod show;
mod stats;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::types::{MessageRow, ToolCallRow};
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};
use systemprompt_models::artifacts::NoticeLine;

pub use stats::{RequestStatsOutput, build_request_stats};

const REQUEST_LIST_COLUMNS: [&str; 8] = [
    "request_id",
    "timestamp",
    "provider",
    "model",
    "tokens",
    "cost",
    "latency_ms",
    "status",
];

#[must_use]
pub fn build_request_list(rows: &[RequestListRow]) -> CommandOutput {
    if rows.is_empty() {
        return CommandOutput::message(vec![NoticeLine::new("info", "No AI requests found")]);
    }
    CommandOutput::table_of(REQUEST_LIST_COLUMNS.to_vec(), rows).with_title("AI Requests")
}

#[must_use]
pub fn build_request_show(detail: &RequestShowOutput) -> CommandOutput {
    CommandOutput::card_value("AI Request Details", detail)
}

#[must_use]
pub fn request_show_not_found(request_id: &str) -> CommandOutput {
    CommandOutput::message(vec![
        NoticeLine::new("warning", format!("AI request not found: {request_id}")),
        NoticeLine::new(
            "info",
            "Tip: Use 'systemprompt infra logs request list' to see recent requests",
        ),
    ])
}

#[derive(Debug, Subcommand)]
pub enum RequestCommands {
    #[command(
        about = "Operational list of recent AI requests. For dashboard metrics (time range, model filter, CSV export), use `analytics requests list`",
        after_help = "EXAMPLES:\n  systemprompt infra logs request list\n  systemprompt infra \
                      logs request list --model gpt-4 --since 1h"
    )]
    List(list::ListArgs),

    #[command(
        about = "Quick single-request view by request id (messages, linked MCP calls, status/error)",
        after_help = "EXAMPLES:\n  systemprompt infra logs request show abc123\n  systemprompt \
                      infra logs request show abc123 --messages --tools"
    )]
    Show(show::ShowArgs),

    #[command(
        about = "Operational request aggregate with by-provider / by-model breakdown. For range/model-filtered dashboards with export, use `analytics requests stats`",
        after_help = "EXAMPLES:\n  systemprompt infra logs request stats\n  systemprompt infra \
                      logs request stats --since 24h"
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

pub async fn execute(command: RequestCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        RequestCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestCommands::Show(args) => {
            let result = show::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestCommands::Stats(args) => stats::execute(args, ctx).await,
    }
}
