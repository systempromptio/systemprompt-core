//! `core contexts new` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

use super::caller_and_pool;
use super::types::ContextSwitchedOutput;

#[derive(Debug, Args)]
pub struct NewArgs {
    #[arg(long, help = "Name for the new context")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: NewArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    let repo = ContextRepository::new(&pool);

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

    host.use_session_context(context_id.clone()).await?;

    let output = ContextSwitchedOutput {
        id: context_id,
        message: format!("Created and switched to context '{}'", name),
        name,
    };

    Ok(CommandOutput::card_value("New Context Created", &output))
}
