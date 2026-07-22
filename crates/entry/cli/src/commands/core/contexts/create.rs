//! `core contexts create` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_logging::CliService;

use super::types::ContextCreatedOutput;
use crate::context::CommandContext;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Name for the new context")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: CreateArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;
    execute_with_pool(args, &session_ctx.session, &pool, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: CreateArgs,
    session: &systemprompt_cloud::CliSession,
    pool: &systemprompt_database::DbPool,
    config: &crate::cli_settings::CliConfig,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool)?;

    let name = args
        .name
        .unwrap_or_else(|| format!("Context - {}", Utc::now().format("%Y-%m-%d %H:%M")));

    let context_id = repo
        .create_context(
            &session.user_id,
            Some(&session.session_id),
            &name,
            ContextKind::User,
        )
        .await
        .context("Failed to create context")?;

    let output = ContextCreatedOutput {
        id: context_id.clone(),
        name: name.clone(),
        message: format!("Context '{}' created successfully", name),
    };

    if !config.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &name);
    }

    Ok(CommandOutput::card_value("Context Created", &output))
}
