use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

use super::resolve::resolve_context;
use super::types::ContextDeletedOutput;
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContextDeletedOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;

    let repo = ContextRepository::new(Arc::clone(ctx.db_pool()));

    let context_id = resolve_context(&args.context, &session_ctx.session.user_id, &repo).await?;

    if context_id == session_ctx.session.context_id {
        bail!(
            "Cannot delete the active context. Switch to a different context first with 'contexts \
             use <id>'."
        );
    }

    let context = repo
        .get_context(&context_id, &session_ctx.session.user_id)
        .await
        .context("Failed to fetch context details")?;

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!(
            "You are about to delete context '{}' ({})",
            context.name,
            context_id.as_str()
        ));

        if !dialoguer::Confirm::new()
            .with_prompt("Are you sure?")
            .default(false)
            .interact()?
        {
            CliService::info("Deletion cancelled");
            return Ok(CommandResult::card(ContextDeletedOutput {
                id: context_id,
                message: "Deletion cancelled".to_string(),
            })
            .with_title("Context Delete Cancelled"));
        }
    }

    repo.delete_context(&context_id, &session_ctx.session.user_id)
        .await
        .context("Failed to delete context")?;

    let output = ContextDeletedOutput {
        id: context_id.clone(),
        message: format!("Context '{}' deleted successfully", context.name),
    };

    if !config.is_json_output() {
        CliService::success(&output.message);
    }

    Ok(CommandResult::card(output).with_title("Context Deleted"))
}
