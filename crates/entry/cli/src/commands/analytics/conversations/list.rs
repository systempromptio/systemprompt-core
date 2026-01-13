use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

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
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_list(&pool, start, end, args.limit).await?;

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

async fn fetch_list(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<ConversationListOutput> {
    let rows: Vec<(
        String,
        Option<String>,
        i64,
        i64,
        DateTime<Utc>,
        DateTime<Utc>,
    )> = sqlx::query_as(
        r"
        SELECT
            uc.context_id,
            uc.name,
            (SELECT COUNT(*) FROM agent_tasks at WHERE at.context_id = uc.context_id) as task_count,
            (SELECT COUNT(*) FROM task_messages tm
             JOIN agent_tasks at ON at.task_id = tm.task_id
             WHERE at.context_id = uc.context_id) as message_count,
            uc.created_at,
            uc.updated_at
        FROM user_contexts uc
        WHERE uc.created_at >= $1 AND uc.created_at < $2
        ORDER BY uc.updated_at DESC
        LIMIT $3
        ",
    )
    .bind(start)
    .bind(end)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await?;

    let conversations: Vec<ConversationListRow> = rows
        .into_iter()
        .map(
            |(context_id, name, task_count, message_count, created_at, updated_at)| {
                ConversationListRow {
                    context_id,
                    name,
                    task_count,
                    message_count,
                    created_at: created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    updated_at: updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                }
            },
        )
        .collect();

    Ok(ConversationListOutput {
        total: conversations.len() as i64,
        conversations,
    })
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
