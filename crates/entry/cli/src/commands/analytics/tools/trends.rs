//! `analytics tools trends` command with chart output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{ToolTrendPoint, ToolTrendsOutput};
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

    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &ToolAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo
        .get_executions_for_trends(start, end, args.tool.as_deref())
        .await?;

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();

    for row in rows {
        let period_key = format_period_label(
            truncate_to_period(row.created_at, &args.group_by),
            &args.group_by,
        );
        let entry = buckets.entry(period_key).or_insert((0, 0, 0));
        entry.0 += 1;
        if row.status.as_deref() == Some("success") {
            entry.1 += 1;
        }
        entry.2 += i64::from(row.execution_time_ms.unwrap_or(0));
    }

    let mut points: Vec<ToolTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (total, successful, exec_time))| {
            let success_rate = if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let avg_time = if total > 0 { exec_time / total } else { 0 };

            ToolTrendPoint {
                timestamp,
                execution_count: total,
                success_rate,
                avg_execution_time_ms: avg_time,
            }
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let output = ToolTrendsOutput {
        tool: args.tool.clone(),
        period: format_date_range(start, end),
        group_by: args.group_by.clone(),
        points,
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

fn build_chart(output: &ToolTrendsOutput) -> ChartArtifact {
    let labels: Vec<String> = output.points.iter().map(|p| p.timestamp.clone()).collect();
    let executions: Vec<f64> = output
        .points
        .iter()
        .map(|p| p.execution_count as f64)
        .collect();
    let success: Vec<f64> = output.points.iter().map(|p| p.success_rate).collect();
    ChartArtifact::new("Tool Usage Trends", ChartType::Line)
        .with_labels(labels)
        .with_datasets(vec![
            ChartDataset::new("executions", executions),
            ChartDataset::new("success_rate", success),
        ])
}
