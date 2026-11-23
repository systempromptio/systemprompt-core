//! `core contexts create` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CallerSession, CommandHost, CommandOutput};

use super::caller_and_pool;
use super::types::ContextCreatedOutput;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Name for the new context")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: CreateArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    execute_with_pool(args, &caller, &pool).await
}

pub async fn execute_with_pool(
    args: CreateArgs,
    caller: &CallerSession,
    pool: &DbPool,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let name = args
        .name
        .unwrap_or_else(|| format!("Context - {}", Utc::now().format("%Y-%m-%d %H:%M")));

    let context_id = repo
        .create_context(
            caller.user_id(),
            Some(caller.session_id()),
            &name,
            ContextKind::User,
        )
        .await
        .context("Failed to create context")?;

    let output = ContextCreatedOutput {
        id: context_id,
        message: format!("Context '{}' created successfully", name),
        name,
    };

    Ok(CommandOutput::card_value("Context Created", &output))
}
