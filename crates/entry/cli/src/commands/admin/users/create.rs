//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::UserService;

use super::types::UserCreatedOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

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

pub(super) async fn execute(args: CreateArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;

    if args.name.trim().is_empty() {
        return Err(anyhow!("Name cannot be empty"));
    }

    if args.email.trim().is_empty() {
        return Err(anyhow!("Email cannot be empty"));
    }

    if args.if_not_exists
        && let Some(existing) = user_service.find_by_name(&args.name).await?
    {
        let output = UserCreatedOutput {
            id: existing.id.clone(),
            name: existing.name.clone(),
            email: existing.email.clone(),
            message: format!("User '{}' already exists", existing.name),
        };
        return Ok(CommandOutput::card_value("User Exists", &output));
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

    Ok(CommandOutput::card_value("User Created", &output))
}
