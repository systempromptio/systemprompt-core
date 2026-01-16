mod breakdown;
mod summary;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum CostsCommands {
    #[command(about = "Cost summary")]
    Summary(summary::SummaryArgs),

    #[command(about = "Cost trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Cost breakdown by model/agent")]
    Breakdown(breakdown::BreakdownArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostSummaryOutput {
    pub period: String,
    pub total_cost_cents: i64,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub avg_cost_per_request_cents: f64,
    pub change_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostTrendPoint {
    pub timestamp: String,
    pub cost_cents: i64,
    pub request_count: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<CostTrendPoint>,
    pub total_cost_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostBreakdownItem {
    pub name: String,
    pub cost_cents: i64,
    pub request_count: i64,
    pub tokens: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostBreakdownOutput {
    pub period: String,
    pub breakdown_by: String,
    pub items: Vec<CostBreakdownItem>,
    pub total_cost_cents: i64,
}

pub async fn execute(command: CostsCommands, config: &CliConfig) -> Result<()> {
    match command {
        CostsCommands::Summary(args) => summary::execute(args, config).await,
        CostsCommands::Trends(args) => trends::execute(args, config).await,
        CostsCommands::Breakdown(args) => breakdown::execute(args, config).await,
    }
}

pub async fn execute_with_pool(
    command: CostsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        CostsCommands::Summary(args) => summary::execute_with_pool(args, db_ctx, config).await,
        CostsCommands::Trends(args) => trends::execute_with_pool(args, db_ctx, config).await,
        CostsCommands::Breakdown(args) => breakdown::execute_with_pool(args, db_ctx, config).await,
    }
}
