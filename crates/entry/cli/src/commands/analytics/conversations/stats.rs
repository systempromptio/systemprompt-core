use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_analytics::ConversationAnalyticsRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::ConversationStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
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

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ConversationAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: StatsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: StatsArgs,
    repo: &ConversationAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Conversation Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

fn render_stats(output: &ConversationStatsOutput) {
    CliService::section(&format!("Conversation Statistics ({})", output.period));

    CliService::key_value("Total Contexts", &format_number(output.total_contexts));
    CliService::key_value("Total Tasks", &format_number(output.total_tasks));
    CliService::key_value("Total Messages", &format_number(output.total_messages));
    CliService::key_value(
        "Avg Messages/Task",
        &format!("{:.1}", output.avg_messages_per_task),
    );
    CliService::key_value(
        "Avg Task Duration",
        &format_duration_ms(output.avg_task_duration_ms),
    );
}
