use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ContentTrendPoint, ContentTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_period_label, parse_time_range, resolve_export_path, truncate_to_period,
};
use crate::shared::{ChartType, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TrendsArgs {
    #[arg(long, default_value = "7d", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, default_value = "day", help = "Group by period")]
    pub group_by: String,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: TrendsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ContentTrendsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ContentTrendsOutput>> {
    let repo = ContentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &ContentAnalyticsRepository,
) -> Result<CommandResult<ContentTrendsOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_content_for_trends(start, end).await?;

    let mut buckets: HashMap<String, (i64, i64)> = HashMap::new();

    for row in rows {
        let key = format_period_label(
            truncate_to_period(row.timestamp, &args.group_by),
            &args.group_by,
        );
        let entry = buckets.entry(key).or_insert((0, 0));
        entry.0 += row.views;
        entry.1 += row.unique_visitors;
    }

    let mut points: Vec<ContentTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (views, visitors))| ContentTrendPoint {
            timestamp,
            views,
            unique_visitors: visitors,
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let output = ContentTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: args.group_by.clone(),
        points,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.points, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::chart(output, ChartType::Line).with_skip_render());
    }

    if output.points.is_empty() {
        CliService::warning("No data found");
        return Ok(CommandResult::chart(output, ChartType::Line).with_skip_render());
    }

    Ok(CommandResult::chart(output, ChartType::Line).with_title("Content Trends"))
}
