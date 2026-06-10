//! Content performance analytics: engagement stats, top content, and trends.
//!
//! Defines the [`ContentCommands`] subcommand tree and the typed output shapes
//! ([`ContentStatsOutput`], [`TopContentOutput`], [`ContentTrendsOutput`])
//! rendered by the `analytics content` commands.

mod stats;
mod top;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "Content engagement statistics", alias = "list")]
    Stats(stats::StatsArgs),

    #[command(about = "Top performing content")]
    Top(top::TopArgs),

    #[command(about = "Top performing content", hide = true)]
    Popular(top::TopArgs),

    #[command(about = "Content trends over time")]
    Trends(trends::TrendsArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentStatsOutput {
    pub period: String,
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: i64,
    pub avg_scroll_depth: f64,
    pub total_clicks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TopContentRow {
    #[serde(rename = "content_id")]
    pub content: String,
    pub slug: String,
    pub title: String,
    pub source: String,
    pub views: i64,
    pub unique_visitors: i64,
    pub avg_time_seconds: i64,
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TopContentOutput {
    pub period: String,
    pub content: Vec<TopContentRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTrendPoint {
    pub timestamp: String,
    pub views: i64,
    pub unique_visitors: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<ContentTrendPoint>,
}

pub async fn execute(command: ContentCommands, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match command {
        ContentCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ContentCommands::Top(args) | ContentCommands::Popular(args) => {
            let result = top::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ContentCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
