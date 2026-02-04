use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService, UserStatus};

use super::list::StatusFilter;
use super::types::UserUpdatedOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct UpdateArgs {
    pub user_id: String,

    #[arg(long)]
    pub email: Option<String>,

    #[arg(long)]
    pub full_name: Option<String>,

    #[arg(long)]
    pub display_name: Option<String>,

    #[arg(long, value_enum)]
    pub status: Option<StatusFilter>,

    #[arg(long)]
    pub email_verified: Option<bool>,
}

pub async fn execute(
    args: UpdateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<UserUpdatedOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(mut user) = existing else {
        return Err(anyhow!("User not found: {}", args.user_id));
    };
    let user_id = user.id.clone();

    let has_updates = args.email.is_some()
        || args.full_name.is_some()
        || args.display_name.is_some()
        || args.status.is_some()
        || args.email_verified.is_some();

    if !has_updates {
        return Err(anyhow!("No fields to update"));
    }

    if let Some(ref email) = args.email {
        user = user_service.update_email(&user_id, email).await?;
    }

    if let Some(ref full_name) = args.full_name {
        user = user_service.update_full_name(&user_id, full_name).await?;
    }

    if let Some(ref display_name) = args.display_name {
        user = user_service
            .update_display_name(&user_id, display_name)
            .await?;
    }

    if let Some(status_filter) = args.status {
        let status: UserStatus = status_filter.into();
        user = user_service.update_status(&user_id, status).await?;
    }

    if let Some(verified) = args.email_verified {
        user = user_service
            .update_email_verified(&user_id, verified)
            .await?;
    }

    let output = UserUpdatedOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        message: format!("User '{}' updated successfully", user.name),
    };

    Ok(CommandResult::text(output).with_title("User Updated"))
}
