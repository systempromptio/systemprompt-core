use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

use super::types::{ContextListOutput, ContextSummary};
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

#[derive(Tabled)]
struct ContextRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Tasks")]
    task_count: i64,
    #[tabled(rename = "Messages")]
    message_count: i64,
    #[tabled(rename = "Updated")]
    updated_at: String,
    #[tabled(rename = "Active")]
    active: String,
}

pub async fn execute(
    _args: ListArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContextListOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;
    execute_with_pool(&session_ctx.session, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    session: &systemprompt_cloud::CliSession,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandResult<ContextListOutput>> {
    let repo = ContextRepository::new(pool)?;

    let contexts = repo
        .list_contexts_with_stats(&session.user_id)
        .await
        .context("Failed to list contexts")?;

    let active_context_id = session.context_id.clone();

    let summaries: Vec<ContextSummary> = contexts
        .into_iter()
        .map(|c| ContextSummary {
            id: c.context_id.clone(),
            name: c.name,
            task_count: c.task_count,
            message_count: c.message_count,
            created_at: c.created_at,
            updated_at: c.updated_at,
            last_message_at: c.last_message_at,
            is_active: c.context_id == active_context_id,
        })
        .collect();

    let total = summaries.len();

    let output = ContextListOutput {
        contexts: summaries.clone(),
        total,
        active_context_id: Some(active_context_id),
    };

    if !config.is_json_output() {
        CliService::section("Contexts");

        if summaries.is_empty() {
            CliService::info("No contexts found");
        } else {
            let rows: Vec<ContextRow> = summaries
                .iter()
                .map(|c| ContextRow {
                    id: c.id.as_str()[..8].to_string(),
                    name: truncate_name(&c.name, 40),
                    task_count: c.task_count,
                    message_count: c.message_count,
                    updated_at: c.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                    active: if c.is_active {
                        "*".to_string()
                    } else {
                        String::new()
                    },
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);

            CliService::info(&format!("Showing {} context(s)", total));
        }
    }

    Ok(CommandResult::table(output)
        .with_title("Contexts")
        .with_columns(vec![
            "id".to_string(),
            "name".to_string(),
            "task_count".to_string(),
            "message_count".to_string(),
            "updated_at".to_string(),
            "is_active".to_string(),
        ]))
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len - 3])
    }
}
