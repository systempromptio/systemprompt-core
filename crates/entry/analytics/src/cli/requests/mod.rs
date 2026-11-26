//! AI request analytics: aggregate stats, individual listings, trends, and
//! model usage.
//!
//! Defines the [`RequestsCommands`] subcommand tree and the typed output shapes
//! ([`RequestStatsOutput`], [`RequestTrendsOutput`], [`ModelsOutput`]) rendered
//! by the `analytics requests` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod models;
mod stats;
mod trends;

use anyhow::Result;
use clap::{Args, Subcommand};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cli::shared::{report_port, with_resolved_export};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_traits::usage_reports::ai::DynAiUsageReport;

/// `analytics requests` with no subcommand runs the stats aggregate.
///
/// That keeps the bare command runnable rather than a usage error. The stats
/// arguments are flattened here, which leaves their clap defaults the single
/// source of truth.
#[derive(Debug, Args)]
pub struct RequestsArgs {
    #[command(subcommand)]
    pub cmd: Option<RequestsCommands>,

    #[command(flatten)]
    pub stats: stats::StatsArgs,
}

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
    pub reasoning_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
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

pub async fn execute(args: RequestsArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    match args.cmd.unwrap_or(RequestsCommands::Stats(args.stats)) {
        RequestsCommands::Stats(args) => {
            let report =
                report_port::<DynAiUsageReport>(host, "analytics requests stats", "gateway")
                    .await?;
            stats::execute(with_resolved_export!(args, host), report.as_ref()).await
        },
        RequestsCommands::List(args) => {
            let report =
                report_port::<DynAiUsageReport>(host, "analytics requests list", "gateway").await?;
            list::execute(with_resolved_export!(args, host), report.as_ref()).await
        },
        RequestsCommands::Trends(args) => {
            let report =
                report_port::<DynAiUsageReport>(host, "analytics requests trends", "gateway")
                    .await?;
            trends::execute(with_resolved_export!(args, host), report.as_ref()).await
        },
        RequestsCommands::Models(args) => {
            let report =
                report_port::<DynAiUsageReport>(host, "analytics requests models", "gateway")
                    .await?;
            models::execute(with_resolved_export!(args, host), report.as_ref()).await
        },
    }
}
