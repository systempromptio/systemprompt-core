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
pub enum AgentsCommands {
    #[command(about = "Aggregate agent statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "List agents with metrics")]
    List(list::ListArgs),

    #[command(about = "Agent usage trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Deep dive into specific agent")]
    Show(show::ShowArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentStatsOutput {
    pub period: String,
    pub total_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub failed_tasks: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
    pub total_ai_requests: i64,
    pub total_cost_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentListRow {
    pub agent_name: String,
    pub task_count: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
    pub total_cost_cents: i64,
    pub last_active: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentListOutput {
    pub agents: Vec<AgentListRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentTrendPoint {
    pub timestamp: String,
    pub task_count: i64,
    pub success_rate: f64,
    pub avg_execution_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentTrendsOutput {
    pub agent: Option<String>,
    pub period: String,
    pub group_by: String,
    pub points: Vec<AgentTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentShowOutput {
    pub agent_name: String,
    pub period: String,
    pub summary: AgentStatsOutput,
    pub status_breakdown: Vec<StatusBreakdownItem>,
    pub top_errors: Vec<ErrorBreakdownItem>,
    pub hourly_distribution: Vec<HourlyDistributionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusBreakdownItem {
    pub status: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBreakdownItem {
    pub error_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct HourlyDistributionItem {
    pub hour: i32,
    pub count: i64,
}

pub async fn execute(command: AgentsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AgentsCommands::Stats(args) => stats::execute(args, config).await,
        AgentsCommands::List(args) => list::execute(args, config).await,
        AgentsCommands::Trends(args) => trends::execute(args, config).await,
        AgentsCommands::Show(args) => show::execute(args, config).await,
    }
}
