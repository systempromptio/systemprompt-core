use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::AgentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::AgentStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_cost, format_duration_ms, format_number, format_percent,
    parse_time_range,
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

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = AgentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = AgentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &AgentAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let stats = repo.get_stats(start, end, args.agent.as_deref()).await?;
    let ai_stats = repo.get_ai_stats(start, end).await?;

    let success_rate = if stats.total_tasks > 0 {
        (stats.completed_tasks as f64 / stats.total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let output = AgentStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_agents: stats.total_agents,
        total_tasks: stats.total_tasks,
        completed_tasks: stats.completed_tasks,
        failed_tasks: stats.failed_tasks,
        success_rate,
        avg_execution_time_ms: stats.avg_execution_time_ms as i64,
        total_ai_requests: ai_stats.total_ai_requests,
        total_cost_microdollars: ai_stats.total_cost_microdollars,
    };

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Agent Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

fn render_stats(output: &AgentStatsOutput) {
    CliService::section(&format!("Agent Statistics ({})", output.period));

    CliService::key_value("Active Agents", &format_number(output.total_agents));
    CliService::key_value("Total Tasks", &format_number(output.total_tasks));
    CliService::key_value("Completed", &format_number(output.completed_tasks));
    CliService::key_value("Failed", &format_number(output.failed_tasks));
    CliService::key_value("Success Rate", &format_percent(output.success_rate));
    CliService::key_value(
        "Avg Execution Time",
        &format_duration_ms(output.avg_execution_time_ms),
    );
    CliService::key_value("AI Requests", &format_number(output.total_ai_requests));
    CliService::key_value("Total Cost", &format_cost(output.total_cost_microdollars));
}
