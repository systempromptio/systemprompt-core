use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{GeoOutput, GeoRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct GeoArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum countries")]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: GeoArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: GeoArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: GeoArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_geo(pool, start, end, args.limit).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.countries, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "country".to_string(),
                "session_count".to_string(),
                "percentage".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Geographic Distribution")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_geo(&output);
    }

    Ok(())
}

async fn fetch_geo(
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<GeoOutput> {
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        r"
        SELECT COALESCE(country, 'Unknown') as country, COUNT(*) as count
        FROM user_sessions
        WHERE started_at >= $1 AND started_at < $2
        GROUP BY country
        ORDER BY COUNT(*) DESC
        LIMIT $3
        ",
    )
    .bind(start)
    .bind(end)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await?;

    let total: i64 = rows.iter().map(|(_, c)| c).sum();

    let countries: Vec<GeoRow> = rows
        .into_iter()
        .map(|(country, count)| {
            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            GeoRow {
                country: country.unwrap_or_else(|| "Unknown".to_string()),
                session_count: count,
                percentage,
            }
        })
        .collect();

    Ok(GeoOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        countries,
        total_sessions: total,
    })
}

fn render_geo(output: &GeoOutput) {
    CliService::section(&format!("Geographic Distribution ({})", output.period));
    CliService::key_value("Total Sessions", &format_number(output.total_sessions));

    for geo in &output.countries {
        CliService::key_value(
            &geo.country,
            &format!(
                "{} ({})",
                format_number(geo.session_count),
                format_percent(geo.percentage)
            ),
        );
    }
}
