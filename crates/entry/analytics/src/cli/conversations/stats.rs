//! `analytics conversations stats` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_logging::CliService;

use super::ConversationStatsOutput;
use crate::cli::shared::{export_single_to_csv, parse_time_range};
use systemprompt_extension::cli::CommandOutput;
use systemprompt_traits::usage_reports::agent::AgentActivityReport;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute(
    args: StatsArgs,
    repo: &dyn AgentActivityReport,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let total_contexts = repo.context_count(start, end).await?;
    let task_stats = repo.task_stats(start, end).await?;
    let total_tasks = task_stats.task_count;
    let total_messages = repo.message_count(start, end).await?;

    let avg_messages_per_task = if total_tasks > 0 {
        total_messages as f64 / total_tasks as f64
    } else {
        0.0
    };

    let output = ConversationStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_contexts,
        total_tasks,
        total_messages,
        avg_messages_per_task,
        avg_task_duration_ms: task_stats.avg_execution_time_ms.map_or(0, |v| v as i64),
    };

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(
            CommandOutput::card_value("Conversation Statistics", &output).with_skip_render(),
        );
    }

    Ok(CommandOutput::card_value(
        "Conversation Statistics",
        &output,
    ))
}
