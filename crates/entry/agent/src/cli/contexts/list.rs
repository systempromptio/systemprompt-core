//! `core contexts list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CallerSession, CommandHost, CommandOutput};

use super::caller_and_pool;
use super::types::{ContextListOutput, ContextSummary};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs;

pub(super) async fn execute(_args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    execute_with_pool(&caller, &pool).await
}

pub async fn execute_with_pool(caller: &CallerSession, pool: &DbPool) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let contexts = repo
        .list_contexts_with_stats(caller.user_id())
        .await
        .context("Failed to list contexts")?;

    let active_context_id = caller.context_id().clone();

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
        contexts: summaries,
        total,
        active_context_id: Some(active_context_id),
    };

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
