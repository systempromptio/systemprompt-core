use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::ContentStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_stats(&pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Content Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

async fn fetch_stats(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ContentStatsOutput> {
    let row: (
        Option<i64>,
        Option<i64>,
        Option<f64>,
        Option<f64>,
        Option<i64>,
    ) = sqlx::query_as(
        r#"
        SELECT
            SUM(total_views),
            SUM(unique_visitors),
            AVG(avg_time_on_page_seconds),
            AVG(avg_scroll_depth),
            SUM(total_clicks)
        FROM (
            SELECT
                content_id,
                COUNT(*) as total_views,
                COUNT(DISTINCT session_id) as unique_visitors,
                AVG(time_on_page_ms) / 1000.0 as avg_time_on_page_seconds,
                AVG(max_scroll_depth) as avg_scroll_depth,
                SUM(click_count) as total_clicks
            FROM engagement_events
            WHERE created_at >= $1 AND created_at < $2
            GROUP BY content_id
        ) subq
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await
    .unwrap_or((None, None, None, None, None));

    Ok(ContentStatsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        total_views: row.0.unwrap_or(0),
        unique_visitors: row.1.unwrap_or(0),
        avg_time_on_page_seconds: row.2.map(|v| v as i64).unwrap_or(0),
        avg_scroll_depth: row.3.unwrap_or(0.0),
        total_clicks: row.4.unwrap_or(0),
    })
}

fn render_stats(output: &ContentStatsOutput) {
    CliService::section(&format!("Content Statistics ({})", output.period));

    CliService::key_value("Total Views", &format_number(output.total_views));
    CliService::key_value("Unique Visitors", &format_number(output.unique_visitors));
    CliService::key_value(
        "Avg Time on Page",
        &format!("{}s", output.avg_time_on_page_seconds),
    );
    CliService::key_value("Avg Scroll Depth", &format_percent(output.avg_scroll_depth));
    CliService::key_value("Total Clicks", &format_number(output.total_clicks));
}
