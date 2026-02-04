mod live;
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
pub enum SessionsCommands {
    #[command(about = "Session statistics")]
    Stats(stats::StatsArgs),

    #[command(about = "Session trends over time")]
    Trends(trends::TrendsArgs),

    #[command(about = "Real-time active sessions")]
    Live(live::LiveArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionStatsOutput {
    pub period: String,
    #[serde(rename = "sessions_created_in_period")]
    pub total_sessions: i64,
    #[serde(rename = "sessions_currently_active")]
    pub active_sessions: i64,
    pub unique_users: i64,
    pub avg_duration_seconds: i64,
    pub avg_requests_per_session: f64,
    pub conversion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrendPoint {
    pub timestamp: String,
    pub session_count: i64,
    pub active_users: i64,
    pub avg_duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrendsOutput {
    pub period: String,
    pub group_by: String,
    pub points: Vec<SessionTrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActiveSessionRow {
    pub session_id: String,
    pub user_type: String,
    pub started_at: String,
    pub duration_seconds: i64,
    pub request_count: i64,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LiveSessionsOutput {
    pub active_count: i64,
    pub sessions: Vec<ActiveSessionRow>,
    pub timestamp: String,
}

pub async fn execute(command: SessionsCommands, config: &CliConfig) -> Result<()> {
    match command {
        SessionsCommands::Stats(args) => {
            let result = stats::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionsCommands::Trends(args) => {
            let result = trends::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionsCommands::Live(args) => {
            let result = live::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub async fn execute_with_pool(
    command: SessionsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        SessionsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionsCommands::Live(args) => {
            let result = live::execute_with_pool(args, db_ctx, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
