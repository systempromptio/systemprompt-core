use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use systemprompt_core_database::DatabaseAdminService;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{PromoteResult, UserAdminService, UserService};
use systemprompt_runtime::AppContext;

use crate::cli_settings::CliConfig;

use super::helpers::format_bytes;
use super::types::{DbAssignAdminOutput, DbMigrateOutput, DbStatusOutput};

pub async fn execute_migrate(config: &CliConfig) -> Result<()> {
    use systemprompt_core_database::Database;
    use systemprompt_loader::ModuleLoader;
    use systemprompt_models::Config;
    use systemprompt_runtime::{install_module_with_db, Modules};

    let sys_config = Config::get()?;

    if config.should_show_verbose() {
        CliService::info(&format!("System path: {}", sys_config.system_path));
        CliService::info(&format!("Database type: {}", sys_config.database_type));
        CliService::info(&format!("Database URL: {}", sys_config.database_url));
    }

    let database = Arc::new(
        Database::from_config(&sys_config.database_type, &sys_config.database_url)
            .await
            .context("Failed to connect to database")?,
    );
    let modules = Modules::from_vec(ModuleLoader::all())?;
    let all_modules = modules.all();

    let mut installed_modules = Vec::new();
    let mut error_count = 0;

    if config.should_show_verbose() {
        CliService::info(&format!("Installing {} modules", all_modules.len()));
    }

    for module in all_modules {
        if config.should_show_verbose() {
            CliService::info(&format!("  Installing: {}", module.name));
        }
        if let Err(e) = install_module_with_db(module, database.as_ref()).await {
            CliService::error(&format!("{} failed: {}", module.name, e));
            error_count += 1;
        } else {
            installed_modules.push(module.name.clone());
        }
    }

    if error_count > 0 {
        return Err(anyhow!("Some modules failed to install"));
    }

    let output = DbMigrateOutput {
        modules_installed: installed_modules,
        message: "Database migration completed successfully".to_string(),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

pub async fn execute_assign_admin(ctx: &AppContext, user: &str, config: &CliConfig) -> Result<()> {
    let user_service = UserService::new(ctx.db_pool())?;
    let user_admin = UserAdminService::new(user_service);

    if !config.is_json_output() {
        CliService::info(&format!("Looking up user: {}", user));
    }

    match user_admin.promote_to_admin(user).await? {
        PromoteResult::Promoted(u, new_roles) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.to_string(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: new_roles.clone(),
                already_admin: false,
                message: format!("Admin role assigned to user '{}' ({})", u.name, u.email),
            };

            if config.is_json_output() {
                CliService::json(&output);
            } else {
                CliService::success(&output.message);
                CliService::info(&format!("   Roles: {:?}", new_roles));
            }
        },
        PromoteResult::AlreadyAdmin(u) => {
            let output = DbAssignAdminOutput {
                user_id: u.id.to_string(),
                name: u.name.clone(),
                email: u.email.clone(),
                roles: u.roles.clone(),
                already_admin: true,
                message: format!("User '{}' already has admin role", u.name),
            };

            if config.is_json_output() {
                CliService::json(&output);
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
        CliService::json(&output);
    } else {
        CliService::success("Database connection: OK");
        CliService::key_value("  Version", &output.version);
        CliService::key_value("  Tables", &output.tables.to_string());
        CliService::key_value("  Size", &output.size);
    }

    Ok(())
}
