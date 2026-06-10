use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
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
    let session_ctx = get_or_create_session(&ctx.cli).await?;
    let pool = ctx.db_pool().await?;

    let repo = ContextRepository::new(&pool)?;

    let name = args
        .name
        .unwrap_or_else(|| format!("Context - {}", Utc::now().format("%Y-%m-%d %H:%M")));

    let context_id = repo
        .create_context(
            &session_ctx.session.user_id,
            Some(&session_ctx.session.session_id),
            &name,
        )
        .await
        .context("Failed to create context")?;

    let output = ContextCreatedOutput {
        id: context_id.clone(),
        name: name.clone(),
        message: format!("Context '{}' created successfully", name),
    };

    if !ctx.cli.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &name);
    }

    Ok(CommandOutput::card_value("Context Created", &output))
}
