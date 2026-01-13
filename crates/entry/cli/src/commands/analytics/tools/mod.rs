mod list;
mod show;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

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
    pub tool_name: String,
    pub server_name: String,
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
    pub tool_name: String,
    pub period: String,
    pub summary: ToolStatsOutput,
    pub status_breakdown: Vec<StatusBreakdownItem>,
    pub top_errors: Vec<ErrorItem>,
    pub usage_by_agent: Vec<AgentUsageItem>,
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
    pub agent_name: String,
    pub count: i64,
    pub percentage: f64,
}

pub async fn execute(command: ToolsCommands, config: &CliConfig) -> Result<()> {
    match command {
        ToolsCommands::Stats(args) => stats::execute(args, config).await,
        ToolsCommands::List(args) => list::execute(args, config).await,
        ToolsCommands::Trends(args) => trends::execute(args, config).await,
        ToolsCommands::Show(args) => show::execute(args, config).await,
    }
}
