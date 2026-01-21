use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_users::{DemoteResult, UserAdminService, UserService};

use crate::commands::admin::users::types::RoleAssignOutput;

#[derive(Debug, Args)]
pub struct DemoteArgs {
    pub identifier: String,
}

pub async fn execute(args: DemoteArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service);

    match admin_service.demote_from_admin(&args.identifier).await? {
        DemoteResult::Demoted(user, new_roles) => {
            let output = RoleAssignOutput {
                id: user.id.clone(),
                name: user.name.clone(),
                roles: new_roles.clone(),
                message: format!("User '{}' demoted from admin", user.name),
            };

            if config.is_json_output() {
                CliService::json(&output);
            } else {
                CliService::success(&output.message);
                CliService::key_value("User", &output.name);
                CliService::key_value("Email", &user.email);
                CliService::key_value("Roles", &new_roles.join(", "));
            }
        },
        DemoteResult::NotAdmin(user) => {
            if config.is_json_output() {
                CliService::json(&serde_json::json!({
                    "id": user.id,
                    "name": user.name,
                    "message": "User is not an admin"
                }));
            } else {
                CliService::warning(&format!("User '{}' is not an admin", user.name));
            }
        },
        DemoteResult::UserNotFound => {
            CliService::error(&format!("User not found: {}", args.identifier));
            return Err(anyhow!("User not found"));
        },
    }

    Ok(())
}
