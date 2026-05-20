use anyhow::{Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserRole, UserService, UserStatus};

use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    #[arg(long, default_value = "admin")]
    pub name: String,

    #[arg(long, default_value = "admin@localhost")]
    pub email: String,

    #[arg(long, default_value = "Platform Admin")]
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BootstrapOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub created: bool,
    pub roles: Vec<String>,
    pub message: String,
}

pub async fn execute(
    args: BootstrapArgs,
    _config: &CliConfig,
) -> Result<CommandResult<BootstrapOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if args.name.trim().is_empty() {
        return Err(anyhow!("Name cannot be empty"));
    }

    let admin_role = UserRole::Admin.as_str().to_string();

    let (user, created) = match user_service.find_by_name(&args.name).await? {
        Some(existing) => (existing, false),
        None => {
            let created = user_service
                .create(&args.name, &args.email, Some(&args.full_name), None)
                .await?;
            (created, true)
        },
    };

    if !user.is_active() {
        return Err(anyhow!(
            "Bootstrap user '{}' exists but has status '{}'; expected '{}'. \
             Re-activate it before running the platform.",
            user.name,
            user.status.as_deref().unwrap_or("(none)"),
            UserStatus::Active.as_str(),
        ));
    }

    let user = if user.roles.contains(&admin_role) {
        user
    } else {
        let mut next_roles = user.roles.clone();
        next_roles.push(admin_role.clone());
        user_service.assign_roles(&user.id, &next_roles).await?
    };

    if !user.roles.contains(&admin_role) {
        return Err(anyhow!(
            "Failed to assign 'admin' role to bootstrap user '{}'",
            user.name
        ));
    }

    let message = if created {
        format!("Bootstrap user '{}' created and granted admin role", user.name)
    } else {
        format!(
            "Bootstrap user '{}' already exists; admin role verified",
            user.name
        )
    };

    let output = BootstrapOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        created,
        roles: user.roles.clone(),
        message,
    };

    let title = if created { "Admin Bootstrapped" } else { "Admin Verified" };
    Ok(CommandResult::text(output).with_title(title))
}
