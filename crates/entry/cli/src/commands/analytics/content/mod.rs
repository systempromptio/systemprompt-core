mod stats;
mod top;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "Content engagement statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "Top performing content")]
    Top(top::TopArgs),

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
    pub content_id: String,
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

pub async fn execute(command: ContentCommands, config: &CliConfig) -> Result<()> {
    match command {
        ContentCommands::Stats(args) => stats::execute(args, config).await,
        ContentCommands::Top(args) => top::execute(args, config).await,
        ContentCommands::Trends(args) => trends::execute(args, config).await,
    }
}

pub async fn execute_with_pool(
    command: ContentCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        ContentCommands::Stats(args) => stats::execute_with_pool(args, db_ctx, config).await,
        ContentCommands::Top(args) => top::execute_with_pool(args, db_ctx, config).await,
        ContentCommands::Trends(args) => trends::execute_with_pool(args, db_ctx, config).await,
    }
}
