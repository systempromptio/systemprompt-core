use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::paths::{get_cloud_paths, CloudPath};
use systemprompt_cloud::CliSession;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

use super::types::ContextSwitchedOutput;
use crate::cli_settings::CliConfig;
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

    let repo = ContextRepository::new(Arc::clone(ctx.db_pool()));

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

    let cloud_paths = get_cloud_paths().context("Failed to resolve cloud paths")?;
    let session_path = cloud_paths.resolve(CloudPath::CliSession);

    let mut session = CliSession::load_from_path(&session_path)
        .context("Failed to load session. Run 'systemprompt cloud login' first.")?;

    session.set_context_id(context_id.clone());
    session
        .save_to_path(&session_path)
        .context("Failed to save session")?;

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
