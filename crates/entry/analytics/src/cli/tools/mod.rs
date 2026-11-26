//! Tool usage analytics: aggregate stats, listings, trends, and per-tool deep
//! dives.
//!
//! Defines the [`ToolsCommands`] subcommand tree and the typed output shapes
//! ([`ToolStatsOutput`], [`ToolListOutput`], [`ToolTrendsOutput`],
//! [`ToolShowOutput`]) rendered by the `analytics tools` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod show;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, McpServerId, McpToolName};

use crate::cli::shared::{report_port, with_resolved_export};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_traits::usage_reports::mcp::DynToolUsageReport;

#[derive(Debug, Subcommand)]
pub enum ToolsCommands {
    #[command(about = "Aggregate tool statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "List tools with metrics")]
    List(list::ListArgs),

    #[command(about = "Tool usage trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Deep dive into specific tool")]
    Show(show::ShowArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolStatsOutput {
    pub period: String,
    pub total_tools: i64,
    pub total_executions: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
    pub p95_execution_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolListRow {
    pub tool_name: McpToolName,
    pub server_name: McpServerId,
    pub execution_count: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
    pub last_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolListOutput {
    pub tools: Vec<ToolListRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolTrendPoint {
    pub timestamp: String,
    pub execution_count: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolTrendsOutput {
    pub tool: Option<String>,
    pub period: String,
    pub group_by: String,
    pub points: Vec<ToolTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolShowOutput {
    pub tool_name: McpToolName,
    pub period: String,
    pub summary: ToolStatsOutput,
    pub status_breakdown: Vec<StatusBreakdownItem>,
    pub top_errors: Vec<ErrorItem>,
    pub usage_by_agent: AgentUsageSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentUsageSection {
    Available { agents: Vec<AgentUsageItem> },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusBreakdownItem {
    pub status: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorItem {
    pub error_message: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentUsageItem {
    pub agent_name: Option<AgentName>,
    pub count: i64,
    pub percentage: f64,
}

pub async fn execute(command: ToolsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    let report = |name: &'static str| report_port::<DynToolUsageReport>(host, name, "mcp");
    match command {
        ToolsCommands::Stats(args) => {
            let port = report("analytics tools stats").await?;
            stats::execute(with_resolved_export!(args, host), port.as_ref()).await
        },
        ToolsCommands::List(args) => {
            let port = report("analytics tools list").await?;
            list::execute(with_resolved_export!(args, host), port.as_ref()).await
        },
        ToolsCommands::Trends(args) => {
            let port = report("analytics tools trends").await?;
            trends::execute(with_resolved_export!(args, host), port.as_ref()).await
        },
        ToolsCommands::Show(args) => {
            let port = report("analytics tools show").await?;
            show::execute(with_resolved_export!(args, host), port.as_ref(), host).await
        },
    }
}
