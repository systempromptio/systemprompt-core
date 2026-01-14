use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{UserAdminService, UserService, UserStatus};
use systemprompt_runtime::AppContext;

use super::list::StatusFilter;
use super::types::UserUpdatedOutput;

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

pub async fn execute(args: UpdateArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(mut user) = existing else {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    };
    let user_id = user.id.clone();

    let has_updates = args.email.is_some()
        || args.full_name.is_some()
        || args.display_name.is_some()
        || args.status.is_some()
        || args.email_verified.is_some();

    if !has_updates {
        CliService::warning("No fields to update");
        return Ok(());
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
        id: user.id.to_string(),
        name: user.name.clone(),
        email: user.email.clone(),
        message: format!("User '{}' updated successfully", user.name),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("ID", &output.id);
        CliService::key_value("Name", &output.name);
        CliService::key_value("Email", &output.email);
    }

    Ok(())
}
