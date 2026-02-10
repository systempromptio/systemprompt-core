use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{SessionKey, SessionStore};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

use super::types::ContextSwitchedOutput;
use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct NewArgs {
    #[arg(long, help = "Name for the new context")]
    pub name: Option<String>,
}

pub async fn execute(
    args: NewArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContextSwitchedOutput>> {
    let session_ctx = get_or_create_session(config).await?;
    let ctx = AppContext::new().await?;

    let repo = ContextRepository::new(ctx.db_pool())?;

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

    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    let session_key = SessionKey::from_tenant_id(
        session_ctx
            .profile
            .cloud
            .as_ref()
            .and_then(|c| c.tenant_id.as_deref()),
    );

    let mut session = session_ctx.session.clone();
    session.set_context_id(context_id.clone());
    store.upsert_session(&session_key, session);
    store.save(&sessions_dir)?;

    let output = ContextSwitchedOutput {
        id: context_id.clone(),
        name: name.clone(),
        message: format!("Created and switched to context '{}'", name),
    };

    if !config.is_json_output() {
        CliService::success(&output.message);
        CliService::key_value("ID", context_id.as_str());
        CliService::key_value("Name", &name);
    }

    Ok(CommandResult::card(output).with_title("New Context Created"))
}
