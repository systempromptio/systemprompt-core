//! `core contexts edit` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_logging::CliService;

use super::resolve::resolve_context;
use super::types::ContextUpdatedOutput;
use crate::context::CommandContext;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(long, help = "New name for the context")]
    pub name: String,
}

pub(super) async fn execute(args: EditArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;

    let repo = ContextRepository::new(&pool)?;

    let context_id = resolve_context(&args.context, &session_ctx.session.user_id, &repo).await?;

    repo.update_context_name(&context_id, &session_ctx.session.user_id, &args.name)
        .await
        .context("Failed to update context")?;

    let output = ContextUpdatedOutput {
        id: context_id.clone(),
        name: args.name.clone(),
        message: format!("Context renamed to '{}'", args.name),
    };

    if !ctx.cli.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &args.name);
    }

    Ok(CommandOutput::card_value("Context Updated", &output))
}
