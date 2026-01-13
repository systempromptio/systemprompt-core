mod models;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RequestsCommands {
    #[command(about = "Aggregate AI request statistics")]
    Stats(stats::StatsArgs),

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
    pub total_cost_cents: i64,
    pub avg_latency_ms: i64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestTrendPoint {
    pub timestamp: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_cents: i64,
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
    pub total_cost_cents: i64,
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
        RequestsCommands::Stats(args) => stats::execute(args, config).await,
        RequestsCommands::Trends(args) => trends::execute(args, config).await,
        RequestsCommands::Models(args) => models::execute(args, config).await,
    }
}
