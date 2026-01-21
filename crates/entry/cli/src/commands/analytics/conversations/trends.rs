use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

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
    let repo = ConversationAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &ConversationAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let context_rows = repo.get_context_timestamps(start, end).await?;
    let task_rows = repo.get_task_timestamps(start, end).await?;
    let message_rows = repo.get_message_timestamps(start, end).await?;

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();

    for row in context_rows {
        let key = format_period_label(
            truncate_to_period(row.timestamp, &args.group_by),
            &args.group_by,
        );
        buckets.entry(key).or_insert((0, 0, 0)).0 += 1;
    }

    for row in task_rows {
        let key = format_period_label(
            truncate_to_period(row.timestamp, &args.group_by),
            &args.group_by,
        );
        buckets.entry(key).or_insert((0, 0, 0)).1 += 1;
    }

    for row in message_rows {
        let key = format_period_label(
            truncate_to_period(row.timestamp, &args.group_by),
            &args.group_by,
        );
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

    let output = ConversationTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: args.group_by.clone(),
        points,
    };

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
