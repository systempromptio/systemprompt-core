use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ConversationListOutput, ConversationListRow};
use crate::commands::analytics::shared::{export_to_csv, format_number, parse_time_range};
use crate::shared::{render_result, CommandResult, RenderingHints};
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

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ConversationAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &ConversationAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_to_csv(&output.conversations, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.conversations.is_empty() {
        CliService::warning("No conversations found");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "context_id".to_string(),
                "name".to_string(),
                "task_count".to_string(),
                "message_count".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Conversations")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_list(&output);
    }

    Ok(())
}

fn render_list(output: &ConversationListOutput) {
    CliService::section("Conversations");

    for conv in &output.conversations {
        let name = conv.name.as_deref().unwrap_or("Unnamed");
        CliService::subsection(&format!("{} ({})", name, &conv.context_id[..8]));
        CliService::key_value("Tasks", &format_number(conv.task_count));
        CliService::key_value("Messages", &format_number(conv.message_count));
        CliService::key_value("Updated", &conv.updated_at);
    }

    CliService::info(&format!("Showing {} conversations", output.total));
}
