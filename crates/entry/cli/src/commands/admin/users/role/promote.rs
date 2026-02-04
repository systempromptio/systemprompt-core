use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct PromoteArgs {
    pub identifier: String,
}

pub async fn execute(
    args: PromoteArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RoleAssignOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service);

    match admin_service.promote_to_admin(&args.identifier).await? {
        PromoteResult::Promoted(user, new_roles) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: new_roles,
                message: format!("User '{}' promoted to admin", user.name),
            };
            Ok(CommandResult::text(output).with_title("User Promoted"))
        },
        PromoteResult::AlreadyAdmin(user) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: user.roles.clone(),
                message: format!("User '{}' is already an admin", user.name),
            };
            Ok(CommandResult::text(output).with_title("User Already Admin"))
        },
        PromoteResult::UserNotFound => Err(anyhow!("User not found: {}", args.identifier)),
    }
}
