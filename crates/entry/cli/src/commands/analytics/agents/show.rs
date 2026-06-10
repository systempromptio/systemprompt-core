use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::{Path, PathBuf};
use systemprompt_analytics::AgentAnalyticsRepository;
use systemprompt_analytics::models::cli::{
    AgentErrorRow, AgentHourlyRow, AgentStatusBreakdownRow, AgentSummaryRow,
};
use systemprompt_logging::CliService;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::DatabaseContext;

use super::{
    AgentShowOutput, AgentStatsOutput, ErrorBreakdownItem, HourlyDistributionItem,
    StatusBreakdownItem,
};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name to analyze")]
    pub agent: String,

    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute(args: ShowArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    let repo = AgentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub(super) async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = AgentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ShowArgs,
    repo: &AgentAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let count = repo.agent_exists(&args.agent, start, end).await?;
    if count == 0 {
        return Ok(no_activity_output(&args.agent));
    }

    let summary_row = repo.get_agent_summary(&args.agent, start, end).await?;
    let status_breakdown_rows = repo.get_status_breakdown(&args.agent, start, end).await?;
    let top_errors_rows = repo.get_top_errors(&args.agent, start, end).await?;
    let hourly_rows = repo
        .get_hourly_distribution(&args.agent, start, end)
        .await?;

    let period = format_period(start, end);
    let output = AgentShowOutput {
        agent_name: args.agent.clone(),
        period: period.clone(),
        summary: build_summary(period, &summary_row),
        status_breakdown: build_status_breakdown(status_breakdown_rows),
        top_errors: build_top_errors(top_errors_rows),
        hourly_distribution: build_hourly_distribution(hourly_rows),
    };

    render_output(args.export.as_deref(), &output)
}

fn no_activity_output(agent: &str) -> CommandOutput {
    CommandOutput::message(vec![
        NoticeLine::new(
            "warning",
            format!(
                "No activity for agent '{}' in the specified time range",
                agent
            ),
        ),
        NoticeLine::new(
            "info",
            "Tip: Use 'systemprompt analytics agents list' to see agents with recent activity",
        ),
    ])
}

fn format_period(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    )
}

fn build_summary(period: String, row: &AgentSummaryRow) -> AgentStatsOutput {
    let success_rate = if row.total_tasks > 0 {
        (row.completed as f64 / row.total_tasks as f64) * 100.0
    } else {
        0.0
    };

    AgentStatsOutput {
        period,
        total_agents: 1,
        total_tasks: row.total_tasks,
        completed_tasks: row.completed,
        failed_tasks: row.failed,
        success_rate,
        avg_execution_time_ms: row.avg_time as i64,
        total_ai_requests: 0,
        total_cost_microdollars: 0,
    }
}

fn build_status_breakdown(rows: Vec<AgentStatusBreakdownRow>) -> Vec<StatusBreakdownItem> {
    let total: i64 = rows.iter().map(|r| r.status_count).sum();
    rows.into_iter()
        .map(|row| StatusBreakdownItem {
            status: row.status,
            count: row.status_count,
            percentage: if total > 0 {
                (row.status_count as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect()
}

fn build_top_errors(rows: Vec<AgentErrorRow>) -> Vec<ErrorBreakdownItem> {
    rows.into_iter()
        .map(|row| ErrorBreakdownItem {
            error_type: row.error_type.unwrap_or_else(|| "Unknown".to_owned()),
            count: row.error_count,
        })
        .collect()
}

fn build_hourly_distribution(rows: Vec<AgentHourlyRow>) -> Vec<HourlyDistributionItem> {
    rows.into_iter()
        .map(|row| HourlyDistributionItem {
            hour: row.task_hour,
            count: row.task_count,
        })
        .collect()
}

fn render_output(export: Option<&Path>, output: &AgentShowOutput) -> Result<CommandOutput> {
    let title = format!("Agent: {}", output.agent_name);

    if let Some(path) = export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::card_value(title, output).with_skip_render());
    }

    Ok(CommandOutput::card_value(title, output))
}
