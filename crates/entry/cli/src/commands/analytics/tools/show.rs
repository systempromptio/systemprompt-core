//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::{Path, PathBuf};
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_analytics::models::cli::{
    ToolAgentUsageRow, ToolErrorRow, ToolStatusBreakdownRow, ToolSummaryRow,
};
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{AgentUsageItem, ErrorItem, StatusBreakdownItem, ToolShowOutput, ToolStatsOutput};
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Tool name to analyze")]
    pub tool: String,

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

pub(super) async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(args: ShowArgs, repo: &ToolAnalyticsRepository) -> Result<CommandOutput> {
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

    let period = format_period(start, end);
    let output = ToolShowOutput {
        tool_name: args.tool.clone(),
        period: period.clone(),
        summary: build_summary(period, &summary_row),
        status_breakdown: build_status_breakdown(status_breakdown_rows),
        top_errors: build_top_errors(top_errors_rows),
        usage_by_agent: build_usage_by_agent(usage_by_agent_rows),
    };

    render_output(args.export.as_deref(), &output)
}

fn format_period(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "{} to {}",
        start.format("%Y-%m-%d %H:%M"),
        end.format("%Y-%m-%d %H:%M")
    )
}

fn build_summary(period: String, row: &ToolSummaryRow) -> ToolStatsOutput {
    let success_rate = if row.total > 0 {
        (row.successful as f64 / row.total as f64) * 100.0
    } else {
        0.0
    };

    ToolStatsOutput {
        period,
        total_tools: 1,
        total_executions: row.total,
        successful: row.successful,
        failed: row.failed,
        timeout: row.timeout,
        success_rate,
        avg_execution_time_ms: row.avg_time as i64,
        p95_execution_time_ms: row.p95_time as i64,
    }
}

fn build_status_breakdown(rows: Vec<ToolStatusBreakdownRow>) -> Vec<StatusBreakdownItem> {
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

fn build_top_errors(rows: Vec<ToolErrorRow>) -> Vec<ErrorItem> {
    rows.into_iter()
        .map(|row| ErrorItem {
            error_message: row.error_msg.unwrap_or_else(|| "Unknown".to_owned()),
            count: row.error_count,
        })
        .collect()
}

fn build_usage_by_agent(rows: Vec<ToolAgentUsageRow>) -> Vec<AgentUsageItem> {
    let agent_total: i64 = rows.iter().map(|r| r.usage_count).sum();
    rows.into_iter()
        .map(|row| AgentUsageItem {
            agent_name: row.agent_name.unwrap_or_else(|| "Direct Call".to_owned()),
            count: row.usage_count,
            percentage: if agent_total > 0 {
                (row.usage_count as f64 / agent_total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect()
}

fn render_output(export: Option<&Path>, output: &ToolShowOutput) -> Result<CommandOutput> {
    let title = format!("Tool: {}", output.tool_name);

    if let Some(path) = export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::card_value(title, output).with_skip_render());
    }

    Ok(CommandOutput::card_value(title, output))
}
