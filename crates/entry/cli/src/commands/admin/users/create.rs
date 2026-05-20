use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::UserCreatedOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub email: String,

    #[arg(long)]
    pub full_name: Option<String>,

    #[arg(long)]
    pub display_name: Option<String>,

    #[arg(long)]
    pub if_not_exists: bool,
}

pub async fn execute(
    args: CreateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<UserCreatedOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if args.name.trim().is_empty() {
        return Err(anyhow!("Name cannot be empty"));
    }

    if args.email.trim().is_empty() {
        return Err(anyhow!("Email cannot be empty"));
    }

    if args.if_not_exists {
        if let Some(existing) = user_service.find_by_name(&args.name).await? {
            let output = UserCreatedOutput {
                id: existing.id.clone(),
                name: existing.name.clone(),
                email: existing.email.clone(),
                message: format!("User '{}' already exists", existing.name),
            };
            return Ok(CommandResult::text(output).with_title("User Exists"));
        }
    }

    let user = user_service
        .create(
            &args.name,
            &args.email,
            args.full_name.as_deref(),
            args.display_name.as_deref(),
        )
        .await?;

    let output = UserCreatedOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        message: format!("User '{}' created successfully", user.name),
    };

    Ok(CommandResult::text(output).with_title("User Created"))
}
