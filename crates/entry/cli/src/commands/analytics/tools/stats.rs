use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ToolStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
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

    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ToolAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Tool Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

fn render_stats(output: &ToolStatsOutput) {
    CliService::section(&format!("Tool Statistics ({})", output.period));

    CliService::key_value("Total Tools", &format_number(output.total_tools));
    CliService::key_value("Total Executions", &format_number(output.total_executions));
    CliService::key_value("Successful", &format_number(output.successful));
    CliService::key_value("Failed", &format_number(output.failed));
    CliService::key_value("Timeout", &format_number(output.timeout));
    CliService::key_value("Success Rate", &format_percent(output.success_rate));
    CliService::key_value(
        "Avg Execution Time",
        &format_duration_ms(output.avg_execution_time_ms),
    );
    CliService::key_value(
        "P95 Execution Time",
        &format_duration_ms(output.p95_execution_time_ms),
    );
}
