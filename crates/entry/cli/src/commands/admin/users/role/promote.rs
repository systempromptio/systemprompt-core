//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct PromoteArgs {
    pub identifier: String,
}

pub(super) async fn execute(args: PromoteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;
    let admin_service = UserAdminService::new(user_service);

    match admin_service.promote_to_admin(&args.identifier).await? {
        PromoteResult::Promoted(user, new_roles) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: new_roles,
                message: format!("User '{}' promoted to admin", user.name),
            };
            Ok(CommandOutput::card_value("User Promoted", &output))
        },
        PromoteResult::AlreadyAdmin(user) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: user.roles.clone(),
                message: format!("User '{}' is already an admin", user.name),
            };
            Ok(CommandOutput::card_value("User Already Admin", &output))
        },
        PromoteResult::UserNotFound => Err(anyhow!("User not found: {}", args.identifier)),
    }
}
