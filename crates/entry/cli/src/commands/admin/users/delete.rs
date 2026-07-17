//! `admin users delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::{UserAdminService, UserService};

use super::types::UserDeletedOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(value_name = "USER_ID")]
    pub user: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    if !args.yes {
        return Err(anyhow!(
            "This will permanently delete the user. Use --yes to confirm."
        ));
    }

    let existing = admin_service.find_user(&args.user).await?;
    let Some(user) = existing else {
        return Err(anyhow!("User not found: {}", args.user));
    };

    user_service.delete(&user.id).await?;

    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!("User '{}' deleted successfully", user.name),
    };

    Ok(CommandOutput::card_value("User Deleted", &output))
}
