use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::CostSummaryOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SummaryArgs {
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

pub async fn execute(
    args: SummaryArgs,
    _config: &CliConfig,
) -> Result<CommandResult<CostSummaryOutput>> {
    let ctx = AppContext::new().await?;
    let repo = CostAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: SummaryArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<CostSummaryOutput>> {
    let repo = CostAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: SummaryArgs,
    repo: &CostAnalyticsRepository,
) -> Result<CommandResult<CostSummaryOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let period_duration = end - start;
    let prev_start = start - period_duration;

    let current = repo.get_summary(start, end).await?;
    let previous = repo.get_previous_cost(prev_start, start).await?;

    let total_cost = current.total_cost.unwrap_or(0);
    let prev_cost = previous.cost.unwrap_or(0);
    let change_percent = if prev_cost > 0 {
        Some(((total_cost - prev_cost) as f64 / prev_cost as f64) * 100.0)
    } else {
        None
    };

    let avg_cost = if current.total_requests > 0 {
        total_cost as f64 / current.total_requests as f64
    } else {
        0.0
    };

    let output = CostSummaryOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_cost_microdollars: total_cost,
        total_requests: current.total_requests,
        total_tokens: current.total_tokens.unwrap_or(0),
        avg_cost_per_request_cents: avg_cost,
        change_percent,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("Cost Summary"))
}
