use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::RequestAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{RequestTrendPoint, RequestTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_period_label, parse_time_range, resolve_export_path, truncate_to_period,
};
use crate::shared::{ChartType, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TrendsArgs {
    #[arg(
        long,
        default_value = "7d",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        default_value = "day",
        help = "Group by period (hour, day, week, month)"
    )]
    pub group_by: String,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: TrendsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RequestTrendsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = RequestAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<RequestTrendsOutput>> {
    let repo = RequestAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &RequestAnalyticsRepository,
) -> Result<CommandResult<RequestTrendsOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_requests_for_trends(start, end).await?;

    let mut buckets: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();

    for row in rows {
        let period_key = format_period_label(
            truncate_to_period(row.created_at, &args.group_by),
            &args.group_by,
        );
        let entry = buckets.entry(period_key).or_insert((0, 0, 0, 0));
        entry.0 += 1;
        entry.1 += i64::from(row.tokens_used.unwrap_or(0));
        entry.2 += row.cost_microdollars.unwrap_or(0);
        entry.3 += i64::from(row.latency_ms.unwrap_or(0));
    }

    let mut points: Vec<RequestTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (count, tokens, cost, latency))| {
            let avg_latency = if count > 0 { latency / count } else { 0 };

            RequestTrendPoint {
                timestamp,
                request_count: count,
                total_tokens: tokens,
                cost_microdollars: cost,
                avg_latency_ms: avg_latency,
            }
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let output = RequestTrendsOutput {
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
        CliService::warning("No data found in the specified time range");
        return Ok(CommandResult::chart(output, ChartType::Line).with_skip_render());
    }

    Ok(CommandResult::chart(output, ChartType::Line).with_title("AI Request Trends"))
}
