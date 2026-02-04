use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::AgentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{
    AgentShowOutput, AgentStatsOutput, ErrorBreakdownItem, HourlyDistributionItem,
    StatusBreakdownItem,
};
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name to analyze")]
    pub agent: String,

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
    args: ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AgentShowOutput>> {
    let ctx = AppContext::new().await?;
    let repo = AgentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<AgentShowOutput>> {
    let repo = AgentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ShowArgs,
    repo: &AgentAnalyticsRepository,
) -> Result<CommandResult<AgentShowOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let count = repo.agent_exists(&args.agent, start, end).await?;
    if count == 0 {
        return Err(anyhow!(
            "Agent '{}' not found in the specified time range",
            args.agent
        ));
    }

    let summary_row = repo.get_agent_summary(&args.agent, start, end).await?;
    let status_breakdown_rows = repo.get_status_breakdown(&args.agent, start, end).await?;
    let top_errors_rows = repo.get_top_errors(&args.agent, start, end).await?;
    let hourly_rows = repo
        .get_hourly_distribution(&args.agent, start, end)
        .await?;

    let success_rate = if summary_row.total_tasks > 0 {
        (summary_row.completed as f64 / summary_row.total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let period = format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    );

    let summary = AgentStatsOutput {
        period: period.clone(),
        total_agents: 1,
        total_tasks: summary_row.total_tasks,
        completed_tasks: summary_row.completed,
        failed_tasks: summary_row.failed,
        success_rate,
        avg_execution_time_ms: summary_row.avg_time as i64,
        total_ai_requests: 0,
        total_cost_microdollars: 0,
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

    let top_errors: Vec<ErrorBreakdownItem> = top_errors_rows
        .into_iter()
        .map(|row| ErrorBreakdownItem {
            error_type: row.error_type.unwrap_or_else(|| "Unknown".to_string()),
            count: row.error_count,
        })
        .collect();

    let hourly_distribution: Vec<HourlyDistributionItem> = hourly_rows
        .into_iter()
        .map(|row| HourlyDistributionItem {
            hour: row.task_hour,
            count: row.task_count,
        })
        .collect();

    let output = AgentShowOutput {
        agent_name: args.agent.clone(),
        period,
        summary,
        status_breakdown,
        top_errors,
        hourly_distribution,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title(format!("Agent: {}", args.agent)))
}
