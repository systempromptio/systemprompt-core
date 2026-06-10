//! AI request analytics: aggregate stats, individual listings, trends, and
//! model usage.
//!
//! Defines the [`RequestsCommands`] subcommand tree and the typed output shapes
//! ([`RequestStatsOutput`], [`RequestTrendsOutput`], [`ModelsOutput`]) rendered
//! by the `analytics requests` commands.

mod list;
mod models;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum RequestsCommands {
    #[command(
        about = "Dashboard request metrics: time range, model filter, cache-hit rate, CSV export. For a quick operational aggregate, use `infra logs request stats`"
    )]
    Stats(stats::StatsArgs),

    #[command(
        about = "Dashboard list of AI requests with time range, model filter, and CSV export. For a quick operational list, use `infra logs request list`"
    )]
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

pub async fn execute(command: RequestsCommands, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match command {
        RequestsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestsCommands::List(args) => {
            let result = list::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RequestsCommands::Models(args) => {
            let result = models::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
