use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::ConversationStatsOutput;
use crate::CliConfig;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ConversationAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let total_contexts = repo.get_context_count(start, end).await?;
    let (total_tasks, avg_duration) = repo.get_task_stats(start, end).await?;
    let total_messages = repo.get_message_count(start, end).await?;

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
        avg_task_duration_ms: avg_duration.map_or(0, |v| v as i64),
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_single_to_csv(&output, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(
            CommandOutput::card_value("Conversation Statistics", &output).with_skip_render(),
        );
    }

    Ok(CommandOutput::card_value(
        "Conversation Statistics",
        &output,
    ))
}
