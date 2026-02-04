use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ConversationListOutput, ConversationListRow};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum conversations"
    )]
    pub limit: i64,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(
    args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ConversationListOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ConversationAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ConversationListOutput>> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &ConversationAnalyticsRepository,
) -> Result<CommandResult<ConversationListOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let rows = repo.list_conversations(start, end, args.limit).await?;

    let conversations: Vec<ConversationListRow> = rows
        .into_iter()
        .map(|row| ConversationListRow {
            context_id: row.context_id.to_string(),
            name: row.name,
            task_count: row.task_count,
            message_count: row.message_count,
            created_at: row.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            updated_at: row.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();

    let output = ConversationListOutput {
        total: conversations.len() as i64,
        conversations,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.conversations, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.conversations.is_empty() {
        CliService::warning("No conversations found");
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "context_id".to_string(),
            "name".to_string(),
            "task_count".to_string(),
            "message_count".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Conversations")
        .with_hints(hints))
}
