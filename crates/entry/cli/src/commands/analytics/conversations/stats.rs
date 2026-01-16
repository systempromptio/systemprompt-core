use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ConversationStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: StatsArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_stats(pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Conversation Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

async fn fetch_stats(
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ConversationStatsOutput> {
    let contexts: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_contexts WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let tasks: (i64, Option<f64>) = sqlx::query_as(
        r"
        SELECT COUNT(*), AVG(execution_time_ms)::float8
        FROM agent_tasks
        WHERE started_at >= $1 AND started_at < $2
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let messages: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_messages WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let avg_messages = if tasks.0 > 0 {
        messages.0 as f64 / tasks.0 as f64
    } else {
        0.0
    };

    Ok(ConversationStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_contexts: contexts.0,
        total_tasks: tasks.0,
        total_messages: messages.0,
        avg_messages_per_task: avg_messages,
        avg_task_duration_ms: tasks.1.map_or(0, |v| v as i64),
    })
}

fn render_stats(output: &ConversationStatsOutput) {
    CliService::section(&format!("Conversation Statistics ({})", output.period));

    CliService::key_value("Total Contexts", &format_number(output.total_contexts));
    CliService::key_value("Total Tasks", &format_number(output.total_tasks));
    CliService::key_value("Total Messages", &format_number(output.total_messages));
    CliService::key_value(
        "Avg Messages/Task",
        &format!("{:.1}", output.avg_messages_per_task),
    );
    CliService::key_value(
        "Avg Task Duration",
        &format_duration_ms(output.avg_task_duration_ms),
    );
}
