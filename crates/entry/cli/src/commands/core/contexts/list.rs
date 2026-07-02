use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;

use super::types::{ContextListOutput, ContextSummary};
use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::presentation::tables::context_list_table;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

pub(super) async fn execute(_args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;
    execute_with_pool(&session_ctx.session, &pool, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    session: &systemprompt_cloud::CliSession,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandOutput> {
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
            CliService::output(&context_list_table(&summaries));

            CliService::info(&format!("Showing {} context(s)", total));
        }
    }

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "name",
            "task_count",
            "message_count",
            "updated_at",
            "is_active",
        ],
        &output.contexts,
    )
    .with_title("Contexts"))
}
