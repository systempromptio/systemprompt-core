use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{AgentUsageItem, ErrorItem, StatusBreakdownItem, ToolShowOutput, ToolStatsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
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

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: ShowArgs,
    repo: &ToolAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title(format!("Tool: {}", args.tool));
        render_result(&result);
    } else {
        render_tool_details(&output);
    }

    Ok(())
}

fn render_tool_details(output: &ToolShowOutput) {
    CliService::section(&format!("Tool: {} ({})", output.tool_name, output.period));

    CliService::subsection("Summary");
    CliService::key_value(
        "Executions",
        &format_number(output.summary.total_executions),
    );
    CliService::key_value("Success Rate", &format_percent(output.summary.success_rate));
    CliService::key_value(
        "Avg Duration",
        &format_duration_ms(output.summary.avg_execution_time_ms),
    );
    CliService::key_value(
        "P95 Duration",
        &format_duration_ms(output.summary.p95_execution_time_ms),
    );

    if !output.status_breakdown.is_empty() {
        CliService::subsection("Status Breakdown");
        for item in &output.status_breakdown {
            CliService::key_value(
                &item.status,
                &format!(
                    "{} ({})",
                    format_number(item.count),
                    format_percent(item.percentage)
                ),
            );
        }
    }

    if !output.top_errors.is_empty() {
        CliService::subsection("Top Errors");
        for item in &output.top_errors {
            CliService::key_value(&item.error_message, &format_number(item.count));
        }
    }

    if !output.usage_by_agent.is_empty() {
        CliService::subsection("Usage by Agent");
        for item in &output.usage_by_agent {
            CliService::key_value(
                &item.agent_name,
                &format!(
                    "{} ({})",
                    format_number(item.count),
                    format_percent(item.percentage)
                ),
            );
        }
    }
}
