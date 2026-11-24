//! `admin users role promote` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_extension::cli::CommandHost;
use systemprompt_users::{PromoteResult, UserAdminService, UserRepository, UserService};

use crate::cli::users::types::RoleAssignOutput;
use systemprompt_extension::cli::{CommandOutput, ConfirmArgs};

#[derive(Debug, Args)]
pub struct PromoteArgs {
    pub identifier: String,

    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

pub(super) async fn execute(args: PromoteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)));
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
