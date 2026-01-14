use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{UserAdminService, UserService};
use systemprompt_runtime::AppContext;

use super::types::UserMergeOutput;

#[derive(Debug, Args)]
pub struct MergeArgs {
    /// Source user (will be deleted after merge)
    #[arg(long)]
    pub source: String,

    /// Target user (will receive source's data)
    #[arg(long)]
    pub target: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: MergeArgs, config: &CliConfig) -> Result<()> {
    if !args.yes {
        CliService::warning(
            "This will merge the source user into the target user and DELETE the source. Use --yes to confirm.",
        );
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    // Find source user
    let source_user = admin_service
        .find_user(&args.source)
        .await?
        .ok_or_else(|| anyhow!("Source user not found: {}", args.source))?;

    // Find target user
    let target_user = admin_service
        .find_user(&args.target)
        .await?
        .ok_or_else(|| anyhow!("Target user not found: {}", args.target))?;

    if source_user.id == target_user.id {
        return Err(anyhow!("Source and target users cannot be the same"));
    }

    // Perform merge
    let result = user_service
        .merge_users(&source_user.id, &target_user.id)
        .await?;

    let output = UserMergeOutput {
        source_id: source_user.id.to_string(),
        target_id: target_user.id.to_string(),
        sessions_transferred: result.sessions_transferred,
        tasks_transferred: result.tasks_transferred,
        message: format!(
            "Merged user '{}' into '{}': {} sessions, {} tasks transferred",
            source_user.name,
            target_user.name,
            result.sessions_transferred,
            result.tasks_transferred
        ),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("Source (deleted)", &source_user.name);
        CliService::key_value("Target", &target_user.name);
        CliService::key_value("Sessions transferred", &output.sessions_transferred.to_string());
        CliService::key_value("Tasks transferred", &output.tasks_transferred.to_string());
    }

    Ok(())
}
