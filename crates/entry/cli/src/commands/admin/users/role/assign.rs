//! `admin users role assign` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::{UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct AssignArgs {
    #[arg(value_name = "USER_ID")]
    pub user: String,

    #[arg(long, value_delimiter = ',')]
    pub roles: Vec<String>,
}

pub(super) async fn execute(args: AssignArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    if args.roles.is_empty() {
        return Err(anyhow!("At least one role must be specified"));
    }

    let existing = admin_service.find_user(&args.user).await?;
    let Some(existing_user) = existing else {
        return Err(anyhow!("User not found: {}", args.user));
    };

    let user = user_service
        .assign_roles(&existing_user.id, &args.roles)
        .await?;

    let output = RoleAssignOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        roles: user.roles.clone(),
        message: format!("Roles assigned to user '{}'", user.name),
    };

    Ok(CommandOutput::card_value("Roles Assigned", &output))
}
