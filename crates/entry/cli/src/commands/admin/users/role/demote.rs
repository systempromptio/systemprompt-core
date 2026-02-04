use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{DemoteResult, UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct DemoteArgs {
    pub identifier: String,
}

pub async fn execute(
    args: DemoteArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RoleAssignOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service);

    match admin_service.demote_from_admin(&args.identifier).await? {
        DemoteResult::Demoted(user, new_roles) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: new_roles,
                message: format!("User '{}' demoted from admin", user.name),
            };
            Ok(CommandResult::text(output).with_title("User Demoted"))
        },
        DemoteResult::NotAdmin(user) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: user.roles.clone(),
                message: format!("User '{}' is not an admin", user.name),
            };
            Ok(CommandResult::text(output).with_title("User Not Admin"))
        },
        DemoteResult::UserNotFound => Err(anyhow!("User not found: {}", args.identifier)),
    }
}
