//! Session analytics: aggregate stats, trends, and a real-time live view.
//!
//! Defines the [`SessionsCommands`] subcommand tree and the typed output shapes
//! ([`SessionStatsOutput`], [`SessionTrendsOutput`], [`LiveSessionsOutput`])
//! rendered by the `analytics sessions` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod live;
mod stats;
mod trends;

use anyhow::Result;
use clap::Subcommand;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SessionsCommands {
    #[command(about = "Session statistics", alias = "list")]
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
    #[serde(rename = "session_id")]
    pub session: String,
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

pub async fn execute(command: SessionsCommands, ctx: &CommandContext) -> Result<()> {
    let db_ctx = ctx.database().await?;
    match command {
        SessionsCommands::Stats(args) => {
            let result = stats::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionsCommands::Trends(args) => {
            let result = trends::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionsCommands::Live(mut args) => {
            if ctx.is_database_scoped() {
                args.no_refresh = true;
            }
            let result = live::execute_with_pool(args, &db_ctx, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
