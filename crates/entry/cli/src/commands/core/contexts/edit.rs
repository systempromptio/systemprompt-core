//! `core contexts edit` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_logging::CliService;

use super::resolve::resolve_context;
use super::types::ContextUpdatedOutput;
use systemprompt_database::pool_from_handle;
use crate::session::get_or_create_session;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(long, help = "New name for the context")]
    pub name: String,
}

pub(super) async fn execute(args: EditArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(host).await?;
    let pool = pool_from_handle(host.db().await?)?;
    execute_with_pool(args, &session_ctx.session, &pool, host.output()).await
}

pub async fn execute_with_pool(
    args: EditArgs,
    session: &systemprompt_cloud::CliSession,
    pool: &systemprompt_database::DbPool,
    config: &systemprompt_extension::cli::CliConfig,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let context_id = resolve_context(&args.context, &session.user_id, &repo).await?;

    repo.update_context_name(&context_id, &session.user_id, &args.name)
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

    Ok(CommandOutput::card_value("Context Updated", &output))
}
