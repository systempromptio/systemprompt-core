use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{AgentUsageItem, ErrorItem, StatusBreakdownItem, ToolShowOutput, ToolStatsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Tool name to analyze")]
    pub tool: String,

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

pub async fn execute(args: ShowArgs, _config: &CliConfig) -> Result<CommandResult<ToolShowOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ToolShowOutput>> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ShowArgs,
    repo: &ToolAnalyticsRepository,
) -> Result<CommandResult<ToolShowOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let count = repo.tool_exists(&args.tool, start, end).await?;
    if count == 0 {
        return Err(anyhow!(
            "Tool '{}' not found in the specified time range",
            args.tool
        ));
    }

    let summary_row = repo.get_tool_summary(&args.tool, start, end).await?;
    let status_breakdown_rows = repo.get_status_breakdown(&args.tool, start, end).await?;
    let top_errors_rows = repo.get_top_errors(&args.tool, start, end).await?;
    let usage_by_agent_rows = repo.get_usage_by_agent(&args.tool, start, end).await?;

    let period = format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    );

    let success_rate = if summary_row.total > 0 {
        (summary_row.successful as f64 / summary_row.total as f64) * 100.0
    } else {
        0.0
    };

    let summary = ToolStatsOutput {
        period: period.clone(),
        total_tools: 1,
        total_executions: summary_row.total,
        successful: summary_row.successful,
        failed: summary_row.failed,
        timeout: summary_row.timeout,
        success_rate,
        avg_execution_time_ms: summary_row.avg_time as i64,
        p95_execution_time_ms: summary_row.p95_time as i64,
    };

    let total: i64 = status_breakdown_rows.iter().map(|r| r.status_count).sum();
    let status_breakdown: Vec<StatusBreakdownItem> = status_breakdown_rows
        .into_iter()
        .map(|row| StatusBreakdownItem {
            status: row.status,
            count: row.status_count,
            percentage: if total > 0 {
                (row.status_count as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();

    let top_errors: Vec<ErrorItem> = top_errors_rows
        .into_iter()
        .map(|row| ErrorItem {
            error_message: row.error_msg.unwrap_or_else(|| "Unknown".to_string()),
            count: row.error_count,
        })
        .collect();

    let agent_total: i64 = usage_by_agent_rows.iter().map(|r| r.usage_count).sum();
    let usage_by_agent: Vec<AgentUsageItem> = usage_by_agent_rows
        .into_iter()
        .map(|row| AgentUsageItem {
            agent_name: row.agent_name.unwrap_or_else(|| "Direct Call".to_string()),
            count: row.usage_count,
            percentage: if agent_total > 0 {
                (row.usage_count as f64 / agent_total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();

    let output = ToolShowOutput {
        tool_name: args.tool.clone(),
        period,
        summary,
        status_breakdown,
        top_errors,
        usage_by_agent,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title(format!("Tool: {}", args.tool)))
}
