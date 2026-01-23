use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use systemprompt_database::{DatabaseAdminService, MigrationService};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};
use systemprompt_users::{PromoteResult, UserAdminService, UserService};

use crate::cli_settings::CliConfig;

use super::helpers::format_bytes;
use super::types::{
    AppliedMigrationInfo, DbAssignAdminOutput, DbMigrateOutput, DbStatusOutput,
    ExtensionMigrationStatus, MigrationHistoryOutput, MigrationStatusOutput,
};
use super::MigrationsCommands;

pub async fn execute_migrate(config: &CliConfig) -> Result<()> {
    use systemprompt_database::{install_extension_schemas, Database};
    use systemprompt_extension::ExtensionRegistry;
    use systemprompt_models::Config;

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

    let registry = ExtensionRegistry::discover();
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    install_extension_schemas(&registry, database.as_ref())
        .await
        .map_err(|e| anyhow!("Schema installation failed: {}", e))?;

    let installed_extensions: Vec<String> = registry
        .schema_extensions()
        .iter()
        .map(|ext| ext.id().to_string())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
        message: "Database migration completed successfully".to_string(),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

pub async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    use systemprompt_database::install_extension_schemas;
    use systemprompt_extension::ExtensionRegistry;

    let database = db_ctx.db_pool();

    let registry = ExtensionRegistry::discover();
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    install_extension_schemas(&registry, database.as_ref())
        .await
        .map_err(|e| anyhow!("Schema installation failed: {}", e))?;

    let installed_extensions: Vec<String> = registry
        .schema_extensions()
        .iter()
        .map(|ext| ext.id().to_string())
        .collect();

    let output = DbMigrateOutput {
        modules_installed: installed_extensions,
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
                user_id: u.id.clone(),
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
                user_id: u.id.clone(),
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

pub async fn execute_migrations(
    ctx: &AppContext,
    cmd: MigrationsCommands,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();

    match cmd {
        MigrationsCommands::Status => {
            execute_migrations_status(db.as_ref(), registry, config).await
        },
        MigrationsCommands::History { extension } => {
            execute_migrations_history(db.as_ref(), registry, &extension, config).await
        },
    }
}

pub async fn execute_migrations_standalone(
    db_ctx: &DatabaseContext,
    cmd: MigrationsCommands,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover();

    match cmd {
        MigrationsCommands::Status => {
            execute_migrations_status(db.as_ref(), &registry, config).await
        },
        MigrationsCommands::History { extension } => {
            execute_migrations_history(db.as_ref(), &registry, &extension, config).await
        },
    }
}

async fn execute_migrations_status(
    db: &dyn systemprompt_database::services::DatabaseProvider,
    registry: &ExtensionRegistry,
    config: &CliConfig,
) -> Result<()> {
    let migration_service = MigrationService::new(db);
    let mut extensions = Vec::new();
    let mut total_pending = 0;
    let mut total_applied = 0;

    for ext in registry.schema_extensions() {
        let status: systemprompt_database::MigrationStatus = migration_service
            .get_migration_status(ext.as_ref())
            .await
            .map_err(|e| anyhow!("Failed to get migration status: {}", e))?;

        total_pending += status.pending_count;
        total_applied += status.total_applied;

        extensions.push(ExtensionMigrationStatus {
            extension_id: status.extension_id,
            is_required: ext.is_required(),
            total_defined: status.total_defined,
            total_applied: status.total_applied,
            pending_count: status.pending_count,
            pending_versions: status.pending.iter().map(|m| m.version).collect(),
        });
    }

    let output = MigrationStatusOutput {
        extensions,
        total_pending,
        total_applied,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        if total_pending == 0 {
            CliService::success("All migrations are up to date");
        } else {
            CliService::warning(&format!("{} pending migration(s)", total_pending));
        }
        CliService::info("");

        for ext in &output.extensions {
            let status_icon = if ext.pending_count == 0 { "✓" } else { "!" };
            let required_tag = if ext.is_required { " [required]" } else { "" };

            CliService::info(&format!(
                "  {} {}{}: {}/{} applied",
                status_icon, ext.extension_id, required_tag, ext.total_applied, ext.total_defined
            ));

            if !ext.pending_versions.is_empty() {
                CliService::info(&format!("      Pending: {:?}", ext.pending_versions));
            }
        }
    }

    Ok(())
}

async fn execute_migrations_history(
    db: &dyn systemprompt_database::services::DatabaseProvider,
    registry: &ExtensionRegistry,
    extension_id: &str,
    config: &CliConfig,
) -> Result<()> {
    let ext = registry
        .get(extension_id)
        .ok_or_else(|| anyhow!("Extension '{}' not found", extension_id))?;

    let migration_service = MigrationService::new(db);
    let applied: Vec<systemprompt_database::AppliedMigration> = migration_service
        .get_applied_migrations(extension_id)
        .await
        .map_err(|e| anyhow!("Failed to get migration history: {}", e))?;

    let migrations: Vec<AppliedMigrationInfo> = applied
        .into_iter()
        .map(|m| AppliedMigrationInfo {
            version: m.version,
            name: m.name,
            checksum: m.checksum,
            applied_at: None,
        })
        .collect();

    let output = MigrationHistoryOutput {
        extension_id: extension_id.to_string(),
        migrations,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::info(&format!("Migration history for '{}':", extension_id));
        CliService::info(&format!("  Version: {}", ext.version()));
        CliService::info(&format!("  Required: {}", ext.is_required()));
        CliService::info("");

        if output.migrations.is_empty() {
            CliService::info("  No migrations applied yet");
        } else {
            for m in &output.migrations {
                CliService::info(&format!(
                    "  v{:03} {} (checksum: {})",
                    m.version,
                    m.name,
                    &m.checksum[..8]
                ));
            }
        }
    }

    Ok(())
}
