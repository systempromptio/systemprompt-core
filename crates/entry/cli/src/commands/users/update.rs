use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{UpdateUserParams, UserService, UserStatus};
use systemprompt_identifiers::UserId;
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

    let user_id = UserId::new(&args.user_id);

    let existing = user_service.find_by_id(&user_id).await?;
    if existing.is_none() {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    }

    let has_updates = args.email.is_some()
        || args.full_name.is_some()
        || args.display_name.is_some()
        || args.status.is_some()
        || args.email_verified.is_some();

    if !has_updates {
        CliService::warning("No fields to update");
        return Ok(());
    }

    let status: Option<UserStatus> = args.status.map(Into::into);
    let status_str = status.map(|s| s.as_str().to_string());

    let params = UpdateUserParams {
        email: args.email.as_deref(),
        full_name: args.full_name.as_deref(),
        display_name: args.display_name.as_deref(),
        status: status_str.as_deref(),
        email_verified: args.email_verified,
    };

    let user = user_service.update_all_fields(&user_id, params).await?;

    let output = UserUpdatedOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        message: format!("User '{}' updated successfully", user.name),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("ID", &output.id.to_string());
        CliService::key_value("Name", &output.name);
        CliService::key_value("Email", &output.email);
    }

    Ok(())
}
