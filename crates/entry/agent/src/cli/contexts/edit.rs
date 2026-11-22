//! `core contexts edit` command.
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
use super::types::ContextUpdatedOutput;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(long, help = "New name for the context")]
    pub name: String,
}

pub(super) async fn execute(args: EditArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    execute_with_pool(args, &caller, &pool).await
}

pub async fn execute_with_pool(
    args: EditArgs,
    caller: &CallerSession,
    pool: &DbPool,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let context_id = resolve_context(&args.context, caller.user_id(), &repo).await?;

    repo.update_context_name(&context_id, caller.user_id(), &args.name)
        .await
        .context("Failed to update context")?;

    let output = ContextUpdatedOutput {
        id: context_id,
        message: format!("Context renamed to '{}'", args.name),
        name: args.name,
    };

    Ok(CommandOutput::card_value("Context Updated", &output))
}
