mod list;
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
    pub context_id: String,
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

pub async fn execute(command: ConversationsCommands, config: &CliConfig) -> Result<()> {
    match command {
        ConversationsCommands::Stats(args) => {
            let result = stats::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        ConversationsCommands::Trends(args) => {
            let result = trends::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        ConversationsCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub async fn execute_with_pool(
    command: ConversationsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        ConversationsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        ConversationsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        ConversationsCommands::List(args) => {
            let result = list::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
