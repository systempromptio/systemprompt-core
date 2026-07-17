//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{CostTrendPoint, CostTrendsOutput};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_to_csv, format_date_range, format_period_label, parse_time_range, resolve_export_path,
    truncate_to_period,
};
use crate::shared::{ChartType, CommandOutput};
use systemprompt_models::artifacts::{ChartArtifact, ChartDataset};

#[derive(Debug, Args)]
pub struct TrendsArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "7d",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
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

pub(super) async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &CostAnalyticsRepository,
) -> Result<CommandOutput> {
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
        let cost = row.cost_microdollars.unwrap_or(0);
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
        period: format_date_range(start, end),
        group_by: args.group_by.clone(),
        points,
        total_cost_microdollars: total_cost,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.points, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::chart(build_chart(&output)).with_skip_render());
    }

    if output.points.is_empty() {
        CliService::warning("No data found in the specified time range");
        return Ok(CommandOutput::chart(build_chart(&output)).with_skip_render());
    }

    Ok(CommandOutput::chart(build_chart(&output)))
}

fn build_chart(output: &CostTrendsOutput) -> ChartArtifact {
    let labels: Vec<String> = output.points.iter().map(|p| p.timestamp.clone()).collect();
    let cost: Vec<f64> = output
        .points
        .iter()
        .map(|p| p.cost_microdollars as f64 / 1_000_000.0)
        .collect();
    let requests: Vec<f64> = output
        .points
        .iter()
        .map(|p| p.request_count as f64)
        .collect();
    ChartArtifact::new("Cost Trends", ChartType::Area)
        .with_labels(labels)
        .with_datasets(vec![
            ChartDataset::new("cost_usd", cost),
            ChartDataset::new("requests", requests),
        ])
}
