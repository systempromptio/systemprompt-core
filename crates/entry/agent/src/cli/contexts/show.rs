//! `core contexts show` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CallerSession, CommandHost, CommandOutput};

use super::caller_and_pool;
use super::resolve::resolve_context;
use super::types::ContextSummary;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,
}

pub(super) async fn execute(args: ShowArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    execute_with_pool(args, &caller, &pool).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    caller: &CallerSession,
    pool: &DbPool,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let context_id = resolve_context(&args.context, caller.user_id(), &repo).await?;

    let context = repo
        .list_contexts_with_stats(caller.user_id())
        .await
        .context("Failed to fetch context")?
        .into_iter()
        .find(|c| c.context_id == context_id)
        .ok_or_else(|| anyhow::anyhow!("Context not found: {}", args.context))?;

    let output = ContextSummary {
        id: context.context_id.clone(),
        name: context.name.clone(),
        task_count: context.task_count,
        message_count: context.message_count,
        created_at: context.created_at,
        updated_at: context.updated_at,
        last_message_at: context.last_message_at,
        is_active: context.context_id == *caller.context_id(),
    };

    Ok(CommandOutput::card_value("Context Details", &output))
}
