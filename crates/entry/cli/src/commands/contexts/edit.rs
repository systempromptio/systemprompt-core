use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_core_agent::repository::context::ContextRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::resolve::resolve_context;
use super::types::ContextUpdatedOutput;
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(long, help = "New name for the context")]
    pub name: String,
}

pub async fn execute(
    args: EditArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContextUpdatedOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;

    let repo = ContextRepository::new(Arc::clone(ctx.db_pool()));

    let context_id = resolve_context(&args.context, &session_ctx.session.user_id, &repo).await?;

    repo.update_context_name(&context_id, &session_ctx.session.user_id, &args.name)
        .await
        .context("Failed to update context")?;

    let output = ContextUpdatedOutput {
        id: context_id.clone(),
        name: args.name.clone(),
        message: format!("Context renamed to '{}'", args.name),
    };

    if !config.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &args.name);
    }

    Ok(CommandResult::card(output).with_title("Context Updated"))
}
