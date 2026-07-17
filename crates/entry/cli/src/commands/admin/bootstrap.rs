//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_users::{User, UserRole, UserService, UserStatus};

use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub email: Option<String>,

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

pub async fn execute(args: BootstrapArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let name = resolve_admin_name(args.name.as_deref())?;

    let email = args
        .email
        .clone()
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| format!("{name}@localhost"));

    let user_service = connect_user_service().await?;

    let (user, created) = if let Some(existing) = user_service.find_by_name(&name).await? {
        (existing, false)
    } else {
        let created = user_service
            .create(&name, &email, Some(&args.full_name), None)
            .await?;
        (created, true)
    };

    if !user.is_active() {
        return Err(anyhow!(
            "Bootstrap user '{}' exists but has status '{}'; expected '{}'. Re-activate it before \
             running the platform.",
            user.name,
            user.status.as_deref().unwrap_or("(none)"),
            UserStatus::Active.as_str(),
        ));
    }

    let user = ensure_admin_role(&user_service, user).await?;

    Ok(build_output(user, created))
}

fn resolve_admin_name(requested: Option<&str>) -> Result<String> {
    let configured = Config::get()?.system_admin_username.clone();
    if configured.trim().is_empty() {
        return Err(anyhow!(
            "Profile is missing `system_admin.username`; cannot run bootstrap"
        ));
    }

    match requested {
        Some(n) if !n.trim().is_empty() => {
            if n != configured {
                return Err(anyhow!(
                    "--name '{}' does not match profile system_admin.username '{}'; refusing to \
                     bootstrap the wrong user",
                    n,
                    configured,
                ));
            }
            Ok(n.to_owned())
        },
        _ => Ok(configured),
    }
}

// Why: bootstrap must run before AppContext::build, because AppContext
// resolution requires the admin row to already exist. Open a database
// pool directly so SystemAdmin does not need to be installed yet.
async fn connect_user_service() -> Result<UserService> {
    let database: DbPool = Arc::new(
        Database::from_config_with_write(
            &Config::get()?.database_type,
            &Config::get()?.database_url,
            Config::get()?.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?,
    );
    Ok(UserService::new(&database)?)
}

async fn ensure_admin_role(user_service: &UserService, user: User) -> Result<User> {
    let admin_role = UserRole::Admin.as_str().to_owned();

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

    Ok(user)
}

fn build_output(user: User, created: bool) -> CommandOutput {
    let message = if created {
        format!(
            "Bootstrap user '{}' created and granted admin role",
            user.name
        )
    } else {
        format!(
            "Bootstrap user '{}' already exists; admin role verified",
            user.name
        )
    };

    let output = BootstrapOutput {
        id: user.id,
        name: user.name,
        email: user.email,
        created,
        roles: user.roles,
        message,
    };

    let title = if created {
        "Admin Bootstrapped"
    } else {
        "Admin Verified"
    };
    CommandOutput::card_value(title, &output)
}
