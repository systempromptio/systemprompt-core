use anyhow::{Result, anyhow};
use systemprompt_database::MigrationService;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::MigrationsCommands;
use super::types::{
    AppliedMigrationInfo, ExtensionMigrationStatus, MigrationHistoryOutput, MigrationStatusOutput,
};

pub async fn execute_migrations(
    ctx: &systemprompt_runtime::AppContext,
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
        let result = CommandResult::table(output)
            .with_title("Migration Status")
            .with_columns(vec![
                "extension_id".into(),
                "total_defined".into(),
                "total_applied".into(),
                "pending_count".into(),
            ]);
        render_result(&result);
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
        let result = CommandResult::table(output)
            .with_title("Migration History")
            .with_columns(vec!["version".into(), "name".into(), "checksum".into()]);
        render_result(&result);
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
