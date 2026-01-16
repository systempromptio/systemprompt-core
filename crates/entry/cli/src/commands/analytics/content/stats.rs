use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

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
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: StatsArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_stats(pool, start, end).await?;

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
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ContentStatsOutput> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint as "total_views!",
            COUNT(DISTINCT ae.session_id)::bigint as "unique_visitors!",
            COALESCE(AVG(ee.time_on_page_ms) / 1000.0, 0)::float8 as "avg_time_on_page_seconds!",
            COALESCE(AVG(ee.max_scroll_depth), 0)::float8 as "avg_scroll_depth!",
            COALESCE(SUM(ee.click_count), 0)::bigint as "total_clicks!"
        FROM analytics_events ae
        LEFT JOIN engagement_events ee ON ae.session_id = ee.session_id
        WHERE ae.event_type = 'page_view'
            AND ae.timestamp >= $1 AND ae.timestamp < $2
        "#,
        start,
        end
    )
    .fetch_one(pool.as_ref())
    .await?;

    Ok(ContentStatsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        total_views: row.total_views,
        unique_visitors: row.unique_visitors,
        avg_time_on_page_seconds: row.avg_time_on_page_seconds as i64,
        avg_scroll_depth: row.avg_scroll_depth,
        total_clicks: row.total_clicks,
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
