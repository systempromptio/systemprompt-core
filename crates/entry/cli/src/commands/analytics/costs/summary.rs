use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::CostSummaryOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_cost, format_number, format_percent, format_tokens,
    parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SummaryArgs {
    #[arg(long, default_value = "24h", help = "Time range (e.g., '1h', '24h', '7d')")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: SummaryArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_summary(&pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Cost Summary");
        render_result(&result);
    } else {
        render_summary(&output);
    }

    Ok(())
}

async fn fetch_summary(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<CostSummaryOutput> {
    let period_duration = end - start;
    let prev_start = start - period_duration;

    let current: (i64, Option<i64>, Option<i64>) = sqlx::query_as(
        r#"
        SELECT COUNT(*), SUM(cost_cents), SUM(tokens_used)
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let previous: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(cost_cents) FROM ai_requests WHERE created_at >= $1 AND created_at < $2",
    )
    .bind(prev_start)
    .bind(start)
    .fetch_one(pool.as_ref())
    .await?;

    let total_cost = current.1.unwrap_or(0);
    let prev_cost = previous.0.unwrap_or(0);
    let change_percent = if prev_cost > 0 {
        Some(((total_cost - prev_cost) as f64 / prev_cost as f64) * 100.0)
    } else {
        None
    };

    let avg_cost = if current.0 > 0 {
        total_cost as f64 / current.0 as f64
    } else {
        0.0
    };

    Ok(CostSummaryOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_cost_cents: total_cost,
        total_requests: current.0,
        total_tokens: current.2.unwrap_or(0),
        avg_cost_per_request_cents: avg_cost,
        change_percent,
    })
}

fn render_summary(output: &CostSummaryOutput) {
    CliService::section(&format!("Cost Summary ({})", output.period));

    CliService::key_value("Total Cost", &format_cost(output.total_cost_cents));
    CliService::key_value("Total Requests", &format_number(output.total_requests));
    CliService::key_value("Total Tokens", &format_tokens(output.total_tokens));
    CliService::key_value(
        "Avg Cost/Request",
        &format_cost(output.avg_cost_per_request_cents as i64),
    );

    if let Some(change) = output.change_percent {
        let sign = if change >= 0.0 { "+" } else { "" };
        CliService::key_value("vs Previous Period", &format!("{}{}", sign, format_percent(change)));
    }
}
