//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::{DemoteResult, UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct DemoteArgs {
    pub identifier: String,
}

pub(super) async fn execute(args: DemoteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;
    let admin_service = UserAdminService::new(user_service);

    match admin_service.demote_from_admin(&args.identifier).await? {
        DemoteResult::Demoted(user, new_roles) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: new_roles,
                message: format!("User '{}' demoted from admin", user.name),
            };
            Ok(CommandOutput::card_value("User Demoted", &output))
        },
        DemoteResult::NotAdmin(user) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: user.roles.clone(),
                message: format!("User '{}' is not an admin", user.name),
            };
            Ok(CommandOutput::card_value("User Not Admin", &output))
        },
        DemoteResult::UserNotFound => Err(anyhow!("User not found: {}", args.identifier)),
    }
}
