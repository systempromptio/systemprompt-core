//! `core contexts new` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{SessionKey, SessionStore};
use systemprompt_logging::CliService;

use super::types::ContextSwitchedOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::paths::ResolvedPaths;
use crate::session::{CliSessionContext, get_or_create_session};
use crate::shared::CommandOutput;
use std::path::Path;
use systemprompt_database::DbPool;

#[derive(Debug, Args)]
pub struct NewArgs {
    #[arg(long, help = "Name for the new context")]
    pub name: Option<String>,
}

pub(super) async fn execute(args: NewArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    let pool = ctx.db_pool().await?;
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    execute_resolved(args, &ctx.cli, &session_ctx, &pool, &sessions_dir).await
}

pub async fn execute_resolved(
    args: NewArgs,
    cli: &CliConfig,
    session_ctx: &CliSessionContext,
    pool: &DbPool,
    sessions_dir: &Path,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool)?;

    let name = args
        .name
        .unwrap_or_else(|| format!("Context - {}", Utc::now().format("%Y-%m-%d %H:%M")));

    let context_id = repo
        .create_context(
            &session_ctx.session.user_id,
            Some(&session_ctx.session.session_id),
            &name,
            ContextKind::User,
        )
        .await
        .context("Failed to create context")?;

    let mut store = SessionStore::load_or_create(sessions_dir)?;

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
    store.save(sessions_dir)?;

    let output = ContextSwitchedOutput {
        id: context_id.clone(),
        name: name.clone(),
        message: format!("Created and switched to context '{}'", name),
    };

    if !cli.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &name);
    }

    Ok(CommandOutput::card_value("New Context Created", &output))
}
