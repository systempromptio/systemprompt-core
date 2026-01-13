use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{RequestTrendPoint, RequestTrendsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_duration_ms, format_number, format_period_label,
    format_tokens, parse_time_range, truncate_to_period,
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

struct RequestRow {
    created_at: DateTime<Utc>,
    tokens_used: Option<i32>,
    cost_cents: Option<i32>,
    latency_ms: Option<i32>,
}

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_trends(&pool, start, end, &args.group_by).await?;

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
        let result = CommandResult::chart(output, ChartType::Line).with_title("AI Request Trends");
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
) -> Result<RequestTrendsOutput> {
    let rows: Vec<RequestRow> = sqlx::query_as!(
        RequestRow,
        r#"
        SELECT
            created_at as "created_at!",
            tokens_used,
            cost_cents,
            latency_ms
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        ORDER BY created_at
        "#,
        start,
        end
    )
    .fetch_all(pool.as_ref())
    .await?;

    let mut buckets: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();

    for row in rows {
        let period_key =
            format_period_label(truncate_to_period(row.created_at, group_by), group_by);
        let entry = buckets.entry(period_key).or_insert((0, 0, 0, 0));
        entry.0 += 1;
        entry.1 += i64::from(row.tokens_used.unwrap_or(0));
        entry.2 += i64::from(row.cost_cents.unwrap_or(0));
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
                cost_cents: cost,
                avg_latency_ms: avg_latency,
            }
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(RequestTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: group_by.to_string(),
        points,
    })
}

fn render_trends(output: &RequestTrendsOutput) {
    CliService::section(&format!("AI Request Trends ({})", output.period));
    CliService::key_value("Grouped by", &output.group_by);

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} requests, {} tokens, {}, avg {}",
            point.timestamp,
            format_number(point.request_count),
            format_tokens(point.total_tokens),
            format_cost(point.cost_cents),
            format_duration_ms(point.avg_latency_ms)
        ));
    }
}
