use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{TopContentOutput, TopContentRow};
use crate::commands::analytics::shared::{export_to_csv, format_number, parse_time_range};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct TopArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum content items"
    )]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: TopArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_top(&pool, start, end, args.limit).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.content, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.content.is_empty() {
        CliService::warning("No content found");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "content_id".to_string(),
                "views".to_string(),
                "unique_visitors".to_string(),
                "avg_time_seconds".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Top Content")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_top(&output);
    }

    Ok(())
}

async fn fetch_top(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<TopContentOutput> {
    let rows: Vec<(String, i64, i64, Option<f64>, Option<String>)> = sqlx::query_as(
        r"
        SELECT
            content_id,
            total_views,
            unique_visitors,
            avg_time_on_page_seconds,
            trend_direction
        FROM content_performance_metrics
        WHERE created_at >= $1 AND created_at < $2
        ORDER BY total_views DESC
        LIMIT $3
        ",
    )
    .bind(start)
    .bind(end)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await?;

    let content: Vec<TopContentRow> = rows
        .into_iter()
        .map(
            |(content_id, views, visitors, avg_time, trend)| TopContentRow {
                content_id,
                views,
                unique_visitors: visitors,
                avg_time_seconds: avg_time.map_or(0, |v| v as i64),
                trend: trend.unwrap_or_else(|| "stable".to_string()),
            },
        )
        .collect();

    Ok(TopContentOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        content,
    })
}

fn render_top(output: &TopContentOutput) {
    CliService::section(&format!("Top Content ({})", output.period));

    for item in &output.content {
        CliService::subsection(&item.content_id);
        CliService::key_value("Views", &format_number(item.views));
        CliService::key_value("Unique Visitors", &format_number(item.unique_visitors));
        CliService::key_value("Avg Time", &format!("{}s", item.avg_time_seconds));
        CliService::key_value("Trend", &item.trend);
    }
}
