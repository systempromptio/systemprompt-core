use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

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

struct CostRow {
    created_at: DateTime<Utc>,
    cost_cents: Option<i32>,
    tokens_used: Option<i32>,
}

pub async fn execute(args: TrendsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
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
        let result = CommandResult::chart(output, ChartType::Area).with_title("Cost Trends");
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
) -> Result<CostTrendsOutput> {
    let rows: Vec<CostRow> = sqlx::query_as!(
        CostRow,
        r#"
        SELECT created_at as "created_at!", cost_cents, tokens_used
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        ORDER BY created_at
        "#,
        start,
        end
    )
    .fetch_all(pool.as_ref())
    .await?;

    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();
    let mut total_cost: i64 = 0;

    for row in rows {
        let period_key =
            format_period_label(truncate_to_period(row.created_at, group_by), group_by);
        let entry = buckets.entry(period_key).or_insert((0, 0, 0));
        let cost = i64::from(row.cost_cents.unwrap_or(0));
        entry.0 += cost;
        entry.1 += 1;
        entry.2 += i64::from(row.tokens_used.unwrap_or(0));
        total_cost += cost;
    }

    let mut points: Vec<CostTrendPoint> = buckets
        .into_iter()
        .map(|(timestamp, (cost, count, tokens))| CostTrendPoint {
            timestamp,
            cost_cents: cost,
            request_count: count,
            tokens,
        })
        .collect();

    points.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(CostTrendsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        group_by: group_by.to_string(),
        points,
        total_cost_cents: total_cost,
    })
}

fn render_trends(output: &CostTrendsOutput) {
    CliService::section(&format!("Cost Trends ({})", output.period));
    CliService::key_value("Total", &format_cost(output.total_cost_cents));
    CliService::key_value("Grouped by", &output.group_by);

    for point in &output.points {
        CliService::info(&format!(
            "{}: {} ({} requests, {} tokens)",
            point.timestamp,
            format_cost(point.cost_cents),
            format_number(point.request_count),
            format_tokens(point.tokens)
        ));
    }
}
