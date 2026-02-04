use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ConversationStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, parse_time_range, resolve_export_path,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: StatsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ConversationStatsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ConversationAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ConversationStatsOutput>> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ConversationAnalyticsRepository,
) -> Result<CommandResult<ConversationStatsOutput>> {
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
        return Ok(CommandResult::card(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("Conversation Statistics"))
}
