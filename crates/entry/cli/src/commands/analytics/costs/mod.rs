//! Cost analytics: spend summary, trends over time, and breakdown by model or
//! agent.
//!
//! Defines the [`CostsCommands`] subcommand tree and the typed output shapes
//! ([`CostSummaryOutput`], [`CostTrendsOutput`], [`CostBreakdownOutput`])
//! rendered by the `analytics costs` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod breakdown;
pub mod summary;
mod trends;

use anyhow::Result;
use clap::{Args, Subcommand};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

/// `analytics costs` with no subcommand runs the summary.
///
/// That keeps the bare command runnable rather than a usage error. The
/// summary's own arguments are flattened here, which leaves their clap defaults
/// the single source of truth.
#[derive(Debug, Args)]
pub struct CostsArgs {
    #[command(subcommand)]
    pub cmd: Option<CostsCommands>,

    #[command(flatten)]
    pub summary: summary::SummaryArgs,
}

#[derive(Debug, Subcommand)]
pub enum CostsCommands {
    #[command(about = "Cost summary", alias = "list")]
    Summary(summary::SummaryArgs),

    #[command(about = "Cost trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Cost breakdown by model/agent")]
    Breakdown(breakdown::BreakdownArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostSummaryOutput {
    pub period: String,
    pub total_cost_microdollars: i64,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub reasoning_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub avg_cost_per_request_microdollars: f64,
    pub change_percent: Option<f64>,
    pub auto_widened_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostTrendPoint {
    pub timestamp: String,
    pub cost_microdollars: i64,
    pub request_count: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<CostTrendPoint>,
    pub total_cost_microdollars: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostBreakdownItem {
    pub name: String,
    pub cost_microdollars: i64,
    pub request_count: i64,
    pub tokens: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CostBreakdownOutput {
    pub period: String,
    pub breakdown_by: String,
    pub items: Vec<CostBreakdownItem>,
    pub total_cost_microdollars: i64,
}

pub async fn execute(args: CostsArgs, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match args.cmd.unwrap_or(CostsCommands::Summary(args.summary)) {
        CostsCommands::Summary(args) => {
            let result = summary::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        CostsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        CostsCommands::Breakdown(args) => {
            let result = breakdown::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
