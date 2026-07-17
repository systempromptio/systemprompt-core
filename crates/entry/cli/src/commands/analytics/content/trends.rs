//! `analytics content trends` command with chart output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{ContentTrendPoint, ContentTrendsOutput};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_to_csv, format_date_range, format_period_label, parse_time_range, resolve_export_path,
    truncate_to_period,
};
use crate::shared::{ChartType, CommandOutput};
use systemprompt_models::artifacts::{ChartArtifact, ChartDataset};

#[derive(Debug, Args)]
pub struct TrendsArgs {
    #[arg(long, alias = "from", default_value = "7d", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time")]
    pub until: Option<String>,

    #[arg(long, default_value = "day", help = "Group by period")]
    pub group_by: String,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ContentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: TrendsArgs,
    repo: &ContentAnalyticsRepository,
) -> Result<CommandOutput> {
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
        CliService::warning("No data found");
        return Ok(CommandOutput::chart(build_chart(&output)).with_skip_render());
    }

    Ok(CommandOutput::chart(build_chart(&output)))
}

fn build_chart(output: &ContentTrendsOutput) -> ChartArtifact {
    let labels: Vec<String> = output.points.iter().map(|p| p.timestamp.clone()).collect();
    let views: Vec<f64> = output.points.iter().map(|p| p.views as f64).collect();
    let visitors: Vec<f64> = output
        .points
        .iter()
        .map(|p| p.unique_visitors as f64)
        .collect();
    ChartArtifact::new("Content Trends", ChartType::Line)
        .with_labels(labels)
        .with_datasets(vec![
            ChartDataset::new("views", views),
            ChartDataset::new("unique_visitors", visitors),
        ])
}
