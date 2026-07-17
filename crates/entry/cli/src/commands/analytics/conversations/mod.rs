//! Conversation analytics: aggregate stats, time-series trends, and listings.
//!
//! Defines the [`ConversationsCommands`] subcommand tree and the typed output
//! shapes ([`ConversationStatsOutput`], [`ConversationTrendsOutput`],
//! [`ConversationListOutput`]) rendered by the `analytics conversations`
//! commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ConversationsCommands {
    #[command(about = "Conversation statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "Conversation trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "List conversations")]
    List(list::ListArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationStatsOutput {
    pub period: String,
    pub total_contexts: i64,
    pub total_tasks: i64,
    pub total_messages: i64,
    pub avg_messages_per_task: f64,
    pub avg_task_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationTrendPoint {
    pub timestamp: String,
    pub context_count: i64,
    pub task_count: i64,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<ConversationTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationListRow {
    #[serde(rename = "context_id")]
    pub context: String,
    pub name: Option<String>,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConversationListOutput {
    pub conversations: Vec<ConversationListRow>,
    pub total: i64,
}

pub async fn execute(command: ConversationsCommands, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match command {
        ConversationsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ConversationsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        ConversationsCommands::List(args) => {
            let result = list::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
