use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use super::types::UserMergeOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct MergeArgs {
    #[arg(long, help = "Source user (will be deleted after merge)")]
    pub source: String,

    #[arg(long, help = "Target user (will receive source's data)")]
    pub target: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(
    args: MergeArgs,
    _config: &CliConfig,
) -> Result<CommandResult<UserMergeOutput>> {
    if !args.yes {
        return Err(anyhow!(
            "This will merge the source user into the target user and DELETE the source. Use \
             --yes to confirm."
        ));
    }

    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    let source_user = admin_service
        .find_user(&args.source)
        .await?
        .ok_or_else(|| anyhow!("Source user not found: {}", args.source))?;

    let target_user = admin_service
        .find_user(&args.target)
        .await?
        .ok_or_else(|| anyhow!("Target user not found: {}", args.target))?;

    if source_user.id == target_user.id {
        return Err(anyhow!("Source and target users cannot be the same"));
    }

    let result = user_service
        .merge_users(&source_user.id, &target_user.id)
        .await?;

    let output = UserMergeOutput {
        source_id: source_user.id.clone(),
        target_id: target_user.id.clone(),
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

    Ok(CommandResult::text(output).with_title("Users Merged"))
}
