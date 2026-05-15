use anyhow::{Context, Result, anyhow};
use systemprompt_database::DatabaseAdminService;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::helpers::format_bytes;
use super::types::{DbAssignAdminOutput, DbStatusOutput};

pub use super::admin_migrate::{execute_migrate, execute_migrate_standalone};
pub use super::admin_migrate_down::{execute_migrate_down, execute_migrate_down_standalone};
pub use super::admin_migrate_plan::{execute_migrate_plan, execute_migrate_plan_standalone};
pub use super::admin_migrate_status::{execute_migrate_status, execute_migrate_status_standalone};
pub use super::admin_migrations::{execute_migrations, execute_migrations_standalone};

pub async fn execute_assign_admin(ctx: &AppContext, user: &str, config: &CliConfig) -> Result<()> {
    let user_service = UserService::new(ctx.db_pool())?;
    let user_admin = UserAdminService::new(user_service);

    if !config.is_json_output() {
        CliService::info(&format!("Looking up user: {}", user));
    }

    match user_admin.promote_to_admin(user).await? {
        PromoteResult::Promoted(u, new_roles) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.clone(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: new_roles.clone(),
                already_admin: false,
                message: format!("Admin role assigned to user '{}' ({})", u.name, u.email),
            };

            if config.is_json_output() {
                let result = CommandResult::text(output).with_title("Database Admin");
                render_result(&result);
            } else {
                CliService::success(&output.message);
                CliService::info(&format!("   Roles: {:?}", new_roles));
            }
        },
        PromoteResult::AlreadyAdmin(u) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.clone(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: u.roles.clone(),
                already_admin: true,
                message: format!("User '{}' already has admin role", u.name),
            };

            if config.is_json_output() {
                let result = CommandResult::text(output).with_title("Database Admin");
                render_result(&result);
            } else {
                CliService::warning(&output.message);
            }
        },
        PromoteResult::UserNotFound => {
            return Err(anyhow!("User '{}' not found", user));
        },
    }

    Ok(())
}

pub async fn execute_status(admin: &DatabaseAdminService, config: &CliConfig) -> Result<()> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to connect to database")?;

    let output = DbStatusOutput {
        status: "connected".to_string(),
        version: info.version.clone(),
        tables: info.tables.len(),
        size: format_bytes(info.size as i64),
    };

    if config.is_json_output() {
        let result = CommandResult::text(output).with_title("Database Admin");
        render_result(&result);
    } else {
        CliService::success("Database connection: OK");
        CliService::key_value("  Version", &output.version);
        CliService::key_value("  Tables", &output.tables.to_string());
        CliService::key_value("  Size", &output.size);
    }

    Ok(())
}
