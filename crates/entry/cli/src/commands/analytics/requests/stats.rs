use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_analytics::RequestAnalyticsRepository;
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
    let repo = RequestAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = RequestAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &RequestAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let row = repo.get_stats(start, end, args.model.as_deref()).await?;

    let cache_hit_rate = if row.total > 0 {
        (row.cache_hits as f64 / row.total as f64) * 100.0
    } else {
        0.0
    };

    let output = RequestStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_requests: row.total,
        total_tokens: row.total_tokens.unwrap_or(0),
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        total_cost_cents: row.cost.unwrap_or(0),
        avg_latency_ms: row.avg_latency.map_or(0, |v| v as i64),
        cache_hit_rate,
    };

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
