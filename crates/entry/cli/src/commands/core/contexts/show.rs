use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

use super::resolve::resolve_context;
use super::types::ContextDetailOutput;
use crate::cli_settings::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,
}

pub async fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContextDetailOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;
    execute_with_pool(args, &session_ctx.session, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    session: &systemprompt_cloud::CliSession,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandResult<ContextDetailOutput>> {
    let repo = ContextRepository::new(pool)?;

    let context_id = resolve_context(&args.context, &session.user_id, &repo).await?;
    let active_context_id = &session.context_id;

    let context = repo
        .list_contexts_with_stats(&session.user_id)
        .await
        .context("Failed to fetch context")?
        .into_iter()
        .find(|c| c.context_id == context_id)
        .ok_or_else(|| anyhow::anyhow!("Context not found: {}", args.context))?;

    let output = ContextDetailOutput {
        id: context.context_id.clone(),
        name: context.name.clone(),
        task_count: context.task_count,
        message_count: context.message_count,
        created_at: context.created_at,
        updated_at: context.updated_at,
        last_message_at: context.last_message_at,
        is_active: context.context_id == *active_context_id,
    };

    if !config.is_json_output() {
        CliService::section("Context Details");
        CliService::key_value("ID", context.context_id.as_str());
        CliService::key_value("Name", &context.name);
        CliService::key_value("Tasks", &context.task_count.to_string());
        CliService::key_value("Messages", &context.message_count.to_string());
        CliService::key_value(
            "Created",
            &context
                .created_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        );
        CliService::key_value(
            "Updated",
            &context
                .updated_at
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        );
        if let Some(last_msg) = context.last_message_at {
            CliService::key_value(
                "Last Message",
                &last_msg.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            );
        }
        CliService::key_value("Active", if output.is_active { "Yes" } else { "No" });
    }

    Ok(CommandResult::card(output).with_title("Context Details"))
}
