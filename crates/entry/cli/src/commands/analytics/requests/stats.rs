use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::RequestStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_cost, format_duration_ms, format_number, format_percent,
    format_tokens, parse_time_range,
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

    #[arg(long, help = "Filter by model")]
    pub model: Option<String>,

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
    let output = fetch_stats(pool, start, end, args.model.as_ref()).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("AI Request Statistics");
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
    model_filter: Option<&String>,
) -> Result<RequestStatsOutput> {
    let row: (
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<f64>,
        i64,
    ) = if let Some(model) = model_filter {
        sqlx::query_as(
            r"
                SELECT
                    COUNT(*) as total,
                    SUM(tokens_used) as total_tokens,
                    SUM(input_tokens) as input_tokens,
                    SUM(output_tokens) as output_tokens,
                    SUM(cost_cents) as cost,
                    AVG(latency_ms)::float8 as avg_latency,
                    COUNT(*) FILTER (WHERE cache_hit = true) as cache_hits
                FROM ai_requests
                WHERE created_at >= $1 AND created_at < $2
                  AND model ILIKE $3
                ",
        )
        .bind(start)
        .bind(end)
        .bind(format!("%{}%", model))
        .fetch_one(pool.as_ref())
        .await?
    } else {
        sqlx::query_as(
            r"
                SELECT
                    COUNT(*) as total,
                    SUM(tokens_used) as total_tokens,
                    SUM(input_tokens) as input_tokens,
                    SUM(output_tokens) as output_tokens,
                    SUM(cost_cents) as cost,
                    AVG(latency_ms)::float8 as avg_latency,
                    COUNT(*) FILTER (WHERE cache_hit = true) as cache_hits
                FROM ai_requests
                WHERE created_at >= $1 AND created_at < $2
                ",
        )
        .bind(start)
        .bind(end)
        .fetch_one(pool.as_ref())
        .await?
    };

    let cache_hit_rate = if row.0 > 0 {
        (row.6 as f64 / row.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(RequestStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_requests: row.0,
        total_tokens: row.1.unwrap_or(0),
        input_tokens: row.2.unwrap_or(0),
        output_tokens: row.3.unwrap_or(0),
        total_cost_cents: row.4.unwrap_or(0),
        avg_latency_ms: row.5.map_or(0, |v| v as i64),
        cache_hit_rate,
    })
}

fn render_stats(output: &RequestStatsOutput) {
    CliService::section(&format!("AI Request Statistics ({})", output.period));

    CliService::key_value("Total Requests", &format_number(output.total_requests));
    CliService::key_value("Total Tokens", &format_tokens(output.total_tokens));
    CliService::key_value("Input Tokens", &format_tokens(output.input_tokens));
    CliService::key_value("Output Tokens", &format_tokens(output.output_tokens));
    CliService::key_value("Total Cost", &format_cost(output.total_cost_cents));
    CliService::key_value("Avg Latency", &format_duration_ms(output.avg_latency_ms));
    CliService::key_value("Cache Hit Rate", &format_percent(output.cache_hit_rate));
}
