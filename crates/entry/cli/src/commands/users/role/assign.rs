use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use crate::commands::users::types::RoleAssignOutput;

#[derive(Debug, Args)]
pub struct AssignArgs {
    pub user_id: String,

    #[arg(long, value_delimiter = ',')]
    pub roles: Vec<String>,
}

pub async fn execute(args: AssignArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if args.roles.is_empty() {
        return Err(anyhow!("At least one role must be specified"));
    }

    let user_id = UserId::new(&args.user_id);

    let existing = user_service.find_by_id(&user_id).await?;
    if existing.is_none() {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    }

    let user = user_service.assign_roles(&user_id, &args.roles).await?;

    let output = RoleAssignOutput {
        id: user.id.to_string(),
        name: user.name.clone(),
        roles: user.roles.clone(),
        message: format!("Roles assigned to user '{}'", user.name),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("User", &output.name);
        CliService::key_value("Roles", &output.roles.join(", "));
    }

    Ok(())
}
