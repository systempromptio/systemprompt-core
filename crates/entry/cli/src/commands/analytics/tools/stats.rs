use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ToolStatsOutput;
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

    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: StatsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ToolStatsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ToolStatsOutput>> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ToolAnalyticsRepository,
) -> Result<CommandResult<ToolStatsOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let row = repo.get_stats(start, end, args.tool.as_deref()).await?;

    let success_rate = if row.total_executions > 0 {
        (row.successful as f64 / row.total_executions as f64) * 100.0
    } else {
        0.0
    };

    let output = ToolStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_tools: row.total_tools,
        total_executions: row.total_executions,
        successful: row.successful,
        failed: row.failed,
        timeout: row.timeout,
        success_rate,
        avg_execution_time_ms: row.avg_time as i64,
        p95_execution_time_ms: row.p95_time as i64,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("Tool Statistics"))
}
