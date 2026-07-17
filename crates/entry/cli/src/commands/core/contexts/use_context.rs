//! `core contexts use` command switching the active context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{SessionKey, SessionStore};
use systemprompt_logging::CliService;

use super::resolve::resolve_context;
use super::types::ContextSwitchedOutput;
use crate::context::CommandContext;
use crate::paths::ResolvedPaths;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct UseArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,
}

pub(super) async fn execute(args: UseArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;

    let repo = ContextRepository::new(&pool)?;

    let context_id = resolve_context(&args.context, &session_ctx.session.user_id, &repo).await?;

    let context = repo
        .get_context(&context_id, &session_ctx.session.user_id)
        .await
        .context("Failed to fetch context details")?;

    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    let session_key = SessionKey::from_tenant_id(
        session_ctx
            .profile
            .cloud
            .as_ref()
            .and_then(|c| c.tenant_id.as_ref()),
    );

    let mut session = session_ctx.session.clone();
    session.set_context_id(context_id.clone());
    store.upsert_session(&session_key, session);
    store.save(&sessions_dir)?;

    let output = ContextSwitchedOutput {
        id: context_id.clone(),
        name: context.name.clone(),
        message: format!("Switched to context '{}'", context.name),
    };

    if !ctx.cli.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &context.name);
    }

    Ok(CommandOutput::card_value("Context Switched", &output))
}
