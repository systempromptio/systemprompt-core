use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::SessionStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
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
    let output = fetch_stats(&pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Session Statistics");
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
) -> Result<SessionStatsOutput> {
    let row: (i64, i64, Option<f64>, Option<f64>, i64) = sqlx::query_as(
        r"
        SELECT
            COUNT(*) as total_sessions,
            COUNT(DISTINCT user_id) as unique_users,
            AVG(duration_seconds)::float8 as avg_duration,
            AVG(request_count)::float8 as avg_requests,
            COUNT(*) FILTER (WHERE converted_at IS NOT NULL) as conversions
        FROM user_sessions
        WHERE started_at >= $1 AND started_at < $2
        ",
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let active: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_sessions WHERE ended_at IS NULL AND last_activity_at >= $1",
    )
    .bind(start)
    .fetch_one(pool.as_ref())
    .await?;

    let conversion_rate = if row.0 > 0 {
        (row.4 as f64 / row.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(SessionStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_sessions: row.0,
        active_sessions: active.0,
        unique_users: row.1,
        avg_duration_seconds: row.2.map_or(0, |v| v as i64),
        avg_requests_per_session: row.3.unwrap_or(0.0),
        conversion_rate,
    })
}

fn render_stats(output: &SessionStatsOutput) {
    CliService::section(&format!("Session Statistics ({})", output.period));

    CliService::key_value("Total Sessions", &format_number(output.total_sessions));
    CliService::key_value("Active Sessions", &format_number(output.active_sessions));
    CliService::key_value("Unique Users", &format_number(output.unique_users));
    CliService::key_value("Avg Duration", &format!("{}s", output.avg_duration_seconds));
    CliService::key_value(
        "Avg Requests/Session",
        &format!("{:.1}", output.avg_requests_per_session),
    );
    CliService::key_value("Conversion Rate", &format_percent(output.conversion_rate));
}
