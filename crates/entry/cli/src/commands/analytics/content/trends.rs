use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ContentTrendPoint, ContentTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_period_label, parse_time_range, truncate_to_period,
};
use crate::shared::{render_result, ChartType, CommandResult};
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

struct EngagementRow {
    created_at: DateTime<Utc>,
    session_id: Option<String>,
}

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: TrendsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: TrendsArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_trends(pool, start, end, &args.group_by).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.points, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::chart(output, ChartType::Line).with_title("Content Trends");
        render_result(&result);
    } else if output.points.is_empty() {
        CliService::warning("No data found");
    } else {
        render_trends(&output);
    }

    Ok(())
}

async fn fetch_trends(
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    group_by: &str,
) -> Result<ContentTrendsOutput> {
    let rows: Vec<EngagementRow> = sqlx::query_as!(
        EngagementRow,
        r#"
        SELECT created_at as "created_at!", session_id
        FROM engagement_events
        WHERE created_at >= $1 AND created_at < $2
        "#,
        start,
        end
    )
    .fetch_all(pool.as_ref())
    .await?;

    let mut buckets: HashMap<String, (i64, std::collections::HashSet<String>)> = HashMap::new();

    for row in rows {
        let key = format_period_label(truncate_to_period(row.created_at, group_by), group_by);
        let entry = buckets
            .entry(key)
            .or_insert_with(|| (0, std::collections::HashSet::new()));
        entry.0 += 1;
        if let Some(session_id) = row.session_id {
            entry.1.insert(session_id);
        }
    }

    let mut points: Vec<ContentTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (views, visitors))| ContentTrendPoint {
            timestamp,
            views,
            unique_visitors: visitors.len() as i64,
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(ContentTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: group_by.to_string(),
        points,
    })
}

fn render_trends(output: &ContentTrendsOutput) {
    CliService::section(&format!("Content Trends ({})", output.period));

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} views, {} unique visitors",
            point.timestamp,
            format_number(point.views),
            format_number(point.unique_visitors)
        ));
    }
}
