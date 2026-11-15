//! Content performance analytics: engagement stats, top content, and trends.
//!
//! Defines the [`ContentCommands`] subcommand tree and the typed output shapes
//! ([`ContentStatsOutput`], [`TopContentOutput`], [`ContentTrendsOutput`])
//! rendered by the `analytics content` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod stats;
mod top;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cli::shared::with_resolved_export;
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "Content engagement statistics", alias = "list")]
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

pub async fn execute(command: ContentCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    let pool = pool_from_handle(host.db().await?)?;
    match command {
        ContentCommands::Stats(args) => {
            let result =
                stats::execute_with_pool(with_resolved_export!(args, host), &pool, host.output())
                    .await?;
            Ok(result)
        },
        ContentCommands::Top(args) => {
            let result =
                top::execute_with_pool(with_resolved_export!(args, host), &pool, host.output())
                    .await?;
            Ok(result)
        },
        ContentCommands::Trends(args) => {
            let result =
                trends::execute_with_pool(with_resolved_export!(args, host), &pool, host.output())
                    .await?;
            Ok(result)
        },
    }
}
