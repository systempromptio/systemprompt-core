//! `analytics agents stats` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_logging::CliService;

use super::AgentStatsOutput;
use crate::cli::shared::{export_single_to_csv, parse_time_range};
use systemprompt_extension::cli::CommandOutput;
use systemprompt_traits::usage_reports::agent::AgentActivityReport;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute(
    args: StatsArgs,
    repo: &dyn AgentActivityReport,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let stats = repo.agent_stats(start, end, args.agent.as_deref()).await?;
    let ai_stats = repo.agent_ai_stats(start, end).await?;

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
        return Ok(CommandOutput::card_value("Agent Statistics", &output).with_skip_render());
    }

    Ok(CommandOutput::card_value("Agent Statistics", &output))
}
