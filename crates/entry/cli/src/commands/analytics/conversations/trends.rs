use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{ConversationTrendPoint, ConversationTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_period_label, parse_time_range, truncate_to_period,
};
use crate::shared::{render_result, ChartType, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TrendsArgs {
    #[arg(long, default_value = "7d", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, default_value = "day", help = "Group by period")]
    pub group_by: String,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_trends(&pool, start, end, &args.group_by).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.points, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.points.is_empty() {
        CliService::warning("No data found");
        return Ok(());
    }

    if config.is_json_output() {
        let result =
            CommandResult::chart(output, ChartType::Line).with_title("Conversation Trends");
        render_result(&result);
    } else {
        render_trends(&output);
    }

    Ok(())
}

async fn fetch_trends(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    group_by: &str,
) -> Result<ConversationTrendsOutput> {
    let contexts: Vec<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT created_at FROM user_contexts WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    let tasks: Vec<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT started_at FROM agent_tasks WHERE started_at >= $1 AND started_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    let messages: Vec<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT created_at FROM task_messages WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();

    for (ts,) in contexts {
        let key = format_period_label(truncate_to_period(ts, group_by), group_by);
        buckets.entry(key).or_insert((0, 0, 0)).0 += 1;
    }

    for (ts,) in tasks {
        let key = format_period_label(truncate_to_period(ts, group_by), group_by);
        buckets.entry(key).or_insert((0, 0, 0)).1 += 1;
    }

    for (ts,) in messages {
        let key = format_period_label(truncate_to_period(ts, group_by), group_by);
        buckets.entry(key).or_insert((0, 0, 0)).2 += 1;
    }

    let mut points: Vec<ConversationTrendPoint> = buckets
        .into_iter()
        .map(
            |(timestamp, (contexts, tasks, messages))| ConversationTrendPoint {
                timestamp,
                context_count: contexts,
                task_count: tasks,
                message_count: messages,
            },
        )
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(ConversationTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: group_by.to_string(),
        points,
    })
}

fn render_trends(output: &ConversationTrendsOutput) {
    CliService::section(&format!("Conversation Trends ({})", output.period));

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} contexts, {} tasks, {} messages",
            point.timestamp,
            format_number(point.context_count),
            format_number(point.task_count),
            format_number(point.message_count)
        ));
    }
}
