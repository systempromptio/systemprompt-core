mod list;
mod models;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RequestsCommands {
    #[command(about = "Aggregate AI request statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "List individual AI requests")]
    List(list::ListArgs),

    #[command(about = "AI request trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Model usage breakdown")]
    Models(models::ModelsArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestStatsOutput {
    pub period: String,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestTrendPoint {
    pub timestamp: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<RequestTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelUsageRow {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelsOutput {
    pub period: String,
    pub models: Vec<ModelUsageRow>,
    pub total_requests: i64,
}

pub async fn execute(command: RequestsCommands, config: &CliConfig) -> Result<()> {
    match command {
        RequestsCommands::Stats(args) => {
            let result = stats::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::Trends(args) => {
            let result = trends::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::Models(args) => {
            let result = models::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub async fn execute_with_pool(
    command: RequestsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        RequestsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::List(args) => {
            let result = list::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        RequestsCommands::Models(args) => {
            let result = models::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
