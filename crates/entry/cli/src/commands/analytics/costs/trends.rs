use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{CostTrendPoint, CostTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_number, format_period_label, format_tokens,
    parse_time_range, truncate_to_period,
};
use crate::shared::{render_result, ChartType, CommandResult};
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

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = CostAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &CostAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_costs_for_trends(start, end).await?;

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();
    let mut total_cost: i64 = 0;

    for row in rows {
        let period_key = format_period_label(
            truncate_to_period(row.created_at, &args.group_by),
            &args.group_by,
        );
        let entry = buckets.entry(period_key).or_insert((0, 0, 0));
        let cost = i64::from(row.cost_microdollars.unwrap_or(0));
        entry.0 += cost;
        entry.1 += 1;
        entry.2 += i64::from(row.tokens_used.unwrap_or(0));
        total_cost += cost;
    }

    let mut points: Vec<CostTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (cost, count, tokens))| CostTrendPoint {
            timestamp,
            cost_microdollars: cost,
            request_count: count,
            tokens,
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let output = CostTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: args.group_by.clone(),
        points,
        total_cost_microdollars: total_cost,
    };

    if let Some(ref path) = args.export {
        export_to_csv(&output.points, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.points.is_empty() {
        CliService::warning("No data found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::chart(output, ChartType::Area).with_title("Cost Trends");
        render_result(&result);
    } else {
        render_trends(&output);
    }

    Ok(())
}

fn render_trends(output: &CostTrendsOutput) {
    CliService::section(&format!("Cost Trends ({})", output.period));
    CliService::key_value("Total", &format_cost(output.total_cost_microdollars));
    CliService::key_value("Grouped by", &output.group_by);

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} ({} requests, {} tokens)",
            point.timestamp,
            format_cost(point.cost_microdollars),
            format_number(point.request_count),
            format_tokens(point.tokens)
        ));
    }
}
