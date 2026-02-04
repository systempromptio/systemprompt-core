use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use super::types::UserDeletedOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub user_id: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteArgs,
    _config: &CliConfig,
) -> Result<CommandResult<UserDeletedOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    if !args.yes {
        return Err(anyhow!(
            "This will permanently delete the user. Use --yes to confirm."
        ));
    }

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(user) = existing else {
        return Err(anyhow!("User not found: {}", args.user_id));
    };

    user_service.delete(&user.id).await?;

    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!("User '{}' deleted successfully", user.name),
    };

    Ok(CommandResult::text(output).with_title("User Deleted"))
}
