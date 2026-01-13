use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{ToolTrendPoint, ToolTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_duration_ms, format_number, format_percent, format_period_label,
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

    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

struct ExecutionRow {
    created_at: DateTime<Utc>,
    status: Option<String>,
    execution_time_ms: Option<i32>,
}

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_trends(&pool, start, end, &args.group_by, &args.tool).await?;

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
        let result = CommandResult::chart(output, ChartType::Line).with_title("Tool Usage Trends");
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
    tool_filter: &Option<String>,
) -> Result<ToolTrendsOutput> {
    let rows: Vec<ExecutionRow> = if let Some(tool) = tool_filter {
        sqlx::query_as!(
            ExecutionRow,
            r#"
            SELECT
                created_at as "created_at!",
                status,
                execution_time_ms
            FROM mcp_tool_executions
            WHERE created_at >= $1 AND created_at < $2
              AND tool_name ILIKE $3
            ORDER BY created_at
            "#,
            start,
            end,
            format!("%{}%", tool)
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            ExecutionRow,
            r#"
            SELECT
                created_at as "created_at!",
                status,
                execution_time_ms
            FROM mcp_tool_executions
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY created_at
            "#,
            start,
            end
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();

    for row in rows {
        let period_key =
            format_period_label(truncate_to_period(row.created_at, group_by), group_by);
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

    Ok(ToolTrendsOutput {
        tool: tool_filter.clone(),
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: group_by.to_string(),
        points,
    })
}

fn render_trends(output: &ToolTrendsOutput) {
    let title = output
        .tool
        .as_ref()
        .map(|t| format!("Tool Trends: {} ({})", t, output.period))
        .unwrap_or_else(|| format!("Tool Trends ({})", output.period));

    CliService::section(&title);
    CliService::key_value("Grouped by", &output.group_by);

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} executions, {} success, avg {}",
            point.timestamp,
            format_number(point.execution_count),
            format_percent(point.success_rate),
            format_duration_ms(point.avg_execution_time_ms)
        ));
    }
}
