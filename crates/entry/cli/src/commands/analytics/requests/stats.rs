use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::RequestAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::RequestStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
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

pub async fn execute(
    args: StatsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RequestStatsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = RequestAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<RequestStatsOutput>> {
    let repo = RequestAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &RequestAnalyticsRepository,
) -> Result<CommandResult<RequestStatsOutput>> {
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
        total_cost_microdollars: row.cost.unwrap_or(0),
        avg_latency_ms: row.avg_latency.map_or(0, |v| v as i64),
        cache_hit_rate,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("AI Request Statistics"))
}
