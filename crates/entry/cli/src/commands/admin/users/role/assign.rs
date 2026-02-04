use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct AssignArgs {
    pub user_id: String,

    #[arg(long, value_delimiter = ',')]
    pub roles: Vec<String>,
}

pub async fn execute(
    args: AssignArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RoleAssignOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    if args.roles.is_empty() {
        return Err(anyhow!("At least one role must be specified"));
    }

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(existing_user) = existing else {
        return Err(anyhow!("User not found: {}", args.user_id));
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

    Ok(CommandResult::text(output).with_title("Roles Assigned"))
}
