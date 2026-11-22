//! `core contexts use` command switching the active context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::caller_and_pool;
use super::resolve::resolve_context;
use super::types::ContextSwitchedOutput;

#[derive(Debug, Args)]
pub struct UseArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,
}

pub(super) async fn execute(args: UseArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    let repo = ContextRepository::new(&pool);

    let context_id = resolve_context(&args.context, caller.user_id(), &repo).await?;

    let context = repo
        .get_context(&context_id, caller.user_id())
        .await
        .context("Failed to fetch context details")?;

    host.use_session_context(context_id.clone()).await?;

    let output = ContextSwitchedOutput {
        id: context_id,
        message: format!("Switched to context '{}'", context.name),
        name: context.name,
    };

    Ok(CommandOutput::card_value("Context Switched", &output))
}
