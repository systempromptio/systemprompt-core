use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{
    Database, MigrationConfig, MigrationService, install_extension_schemas_full,
};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::types::{
    DbMigrateDownOutput, DbMigrateOutput, MigratePlanOutput, MigrateStatusOutput, MigrateStatusRow,
    MigrationDriftInfo, PendingMigrationInfo,
};

pub async fn execute_migrate(config: &CliConfig, allow_checksum_drift: bool) -> Result<()> {
    let sys_config = Config::get()?;

    if config.should_show_verbose() {
        CliService::info(&format!("System path: {}", sys_config.system_path));
        CliService::info(&format!("Database type: {}", sys_config.database_type));
        CliService::info(&format!("Database URL: {}", sys_config.database_url));
    }

    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_install(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
        allow_checksum_drift,
    )
    .await
}

pub async fn execute_migrate_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    allow_checksum_drift: bool,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_install(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
        allow_checksum_drift,
    )
    .await
}

pub async fn execute_migrate_down(config: &CliConfig, extension: &str, count: u32) -> Result<()> {
    let sys_config = Config::get()?;

    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    run_down(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
        extension,
        count,
    )
    .await
}

pub async fn execute_migrate_down_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    extension: &str,
    count: u32,
) -> Result<()> {
    let database = db_ctx.db_pool();
    run_down(
        &ExtensionRegistry::discover(),
        database.write_provider(),
        config,
        extension,
        count,
    )
    .await
}

async fn run_down(
    registry: &ExtensionRegistry,
    write_provider: &dyn systemprompt_database::services::DatabaseProvider,
    config: &CliConfig,
    extension_id: &str,
    count: u32,
) -> Result<()> {
    let ext = registry
        .get(extension_id)
        .ok_or_else(|| anyhow!("Extension '{}' not found", extension_id))?;

    let migration_service = MigrationService::new(write_provider);
    let result = migration_service
        .run_down_migrations(ext.as_ref(), count)
        .await
        .map_err(|e| anyhow!("Down migration failed: {}", e))?;

    let output = DbMigrateDownOutput {
        extension: extension_id.to_string(),
        migrations_reverted: result.migrations_run,
        message: format!(
            "Reverted {} migration(s) for '{}'",
            result.migrations_run, extension_id
        ),
    };

    if config.is_json_output() {
        let result = CommandResult::text(output).with_title("Database Admin");
        render_result(&result);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

async fn run_install(
    registry: &ExtensionRegistry,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    allow_checksum_drift: bool,
) -> Result<()> {
    let extension_count = registry.schema_extensions().len();

    if config.should_show_verbose() {
        CliService::info(&format!(
            "Installing schemas for {} extensions",
            extension_count
        ));
    }

    let migration_config = MigrationConfig {
        allow_checksum_drift,
    };

    install_extension_schemas_full(registry, write_provider, &[], migration_config)
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
        let result = CommandResult::text(output).with_title("Database Admin");
        render_result(&result);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}

pub async fn execute_migrate_plan(
    ctx: &AppContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_plan(db.as_ref(), registry, extension, json, config).await
}

pub async fn execute_migrate_plan_standalone(
    db_ctx: &DatabaseContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover();
    run_migrate_plan(db.as_ref(), &registry, extension, json, config).await
}

pub async fn execute_migrate_status(
    ctx: &AppContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_status(db.as_ref(), registry, extension, json, config).await
}

pub async fn execute_migrate_status_standalone(
    db_ctx: &DatabaseContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover();
    run_migrate_status(db.as_ref(), &registry, extension, json, config).await
}

fn select_extensions(
    registry: &ExtensionRegistry,
    extension: Option<&str>,
) -> Result<Vec<Arc<dyn systemprompt_extension::Extension>>> {
    let all = registry.schema_extensions();
    if let Some(ext_id) = extension {
        let filtered: Vec<_> = all.into_iter().filter(|e| e.id() == ext_id).collect();
        if filtered.is_empty() {
            return Err(anyhow!("Extension '{}' not found or has no schema", ext_id));
        }
        Ok(filtered)
    } else {
        Ok(all)
    }
}

async fn run_migrate_plan(
    db: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let extensions = select_extensions(registry, extension)?;
    let migration_service = MigrationService::new(db);

    let mut pending_rows: Vec<PendingMigrationInfo> = Vec::new();
    for ext in &extensions {
        let pending = migration_service
            .plan_pending(ext.as_ref())
            .await
            .map_err(|e| anyhow!("Failed to plan migrations: {}", e))?;
        for p in pending {
            pending_rows.push(PendingMigrationInfo {
                extension_id: p.extension_id,
                version: p.version,
                name: p.name,
                checksum: p.checksum,
                no_tx: p.no_tx,
            });
        }
    }

    let total_pending = pending_rows.len();
    let output = MigratePlanOutput {
        pending: pending_rows,
        total_pending,
    };

    if json || config.is_json_output() {
        let result = CommandResult::table(output)
            .with_title("Migration Plan")
            .with_columns(vec![
                "extension_id".into(),
                "version".into(),
                "name".into(),
                "checksum".into(),
                "no_tx".into(),
            ]);
        render_result(&result);
    } else if output.pending.is_empty() {
        CliService::success("No pending migrations");
    } else {
        CliService::info(&format!("{} pending migration(s):", output.total_pending));
        CliService::info("");
        CliService::info(&format!(
            "  {:<24} {:>7} {:<32} {:<10} {}",
            "EXTENSION", "VERSION", "NAME", "CHECKSUM", "NO_TX"
        ));
        for p in &output.pending {
            let checksum_short = if p.checksum.len() >= 8 {
                &p.checksum[..8]
            } else {
                &p.checksum
            };
            CliService::info(&format!(
                "  {:<24} {:>7} {:<32} {:<10} {}",
                p.extension_id, p.version, p.name, checksum_short, p.no_tx
            ));
        }
    }

    Ok(())
}

async fn run_migrate_status(
    db: &dyn DatabaseProvider,
    registry: &ExtensionRegistry,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let extensions = select_extensions(registry, extension)?;
    let migration_service = MigrationService::new(db);

    let mut rows: Vec<MigrateStatusRow> = Vec::new();
    let mut drift_rows: Vec<MigrationDriftInfo> = Vec::new();
    let mut total_applied = 0usize;
    let mut total_pending = 0usize;

    for ext in &extensions {
        let status = migration_service
            .status(ext.as_ref())
            .await
            .map_err(|e| anyhow!("Failed to get migration status: {}", e))?;

        let drift_versions: std::collections::HashSet<u32> =
            status.drift.iter().map(|d| d.version).collect();

        for a in &status.applied {
            let label = if drift_versions.contains(&a.version) {
                "drift"
            } else {
                "applied"
            };
            rows.push(MigrateStatusRow {
                extension_id: status.extension_id.clone(),
                version: a.version,
                name: a.name.clone(),
                status: label.to_string(),
                applied_at: a.applied_at.clone(),
            });
        }
        for p in &status.pending {
            rows.push(MigrateStatusRow {
                extension_id: status.extension_id.clone(),
                version: p.version,
                name: p.name.clone(),
                status: "pending".to_string(),
                applied_at: None,
            });
        }
        for d in status.drift {
            drift_rows.push(MigrationDriftInfo {
                extension_id: d.extension_id,
                version: d.version,
                name: d.name,
                stored_checksum: d.stored_checksum,
                current_checksum: d.current_checksum,
            });
        }

        total_applied += status.applied.len();
        total_pending += status.pending.len();
    }

    rows.sort_by(|a, b| {
        a.extension_id
            .cmp(&b.extension_id)
            .then(a.version.cmp(&b.version))
    });

    let total_drift = drift_rows.len();
    let output = MigrateStatusOutput {
        rows,
        drift: drift_rows,
        total_applied,
        total_pending,
        total_drift,
    };

    if json || config.is_json_output() {
        let result = CommandResult::table(output)
            .with_title("Migration Status")
            .with_columns(vec![
                "extension_id".into(),
                "version".into(),
                "name".into(),
                "status".into(),
                "applied_at".into(),
            ]);
        render_result(&result);
    } else {
        CliService::info(&format!(
            "Applied: {} | Pending: {} | Drift: {}",
            output.total_applied, output.total_pending, output.total_drift
        ));
        CliService::info("");
        CliService::info(&format!(
            "  {:<24} {:>7} {:<32} {:<10} {}",
            "EXTENSION", "VERSION", "NAME", "STATUS", "APPLIED_AT"
        ));
        for r in &output.rows {
            let applied_at = r.applied_at.as_deref().unwrap_or("-");
            CliService::info(&format!(
                "  {:<24} {:>7} {:<32} {:<10} {}",
                r.extension_id, r.version, r.name, r.status, applied_at
            ));
        }

        if !output.drift.is_empty() {
            CliService::info("");
            CliService::warning(&format!(
                "{} checksum drift(s) detected:",
                output.total_drift
            ));
            for d in &output.drift {
                CliService::info(&format!(
                    "  {} v{:03} {}: stored={} current={}",
                    d.extension_id,
                    d.version,
                    d.name,
                    &d.stored_checksum[..d.stored_checksum.len().min(8)],
                    &d.current_checksum[..d.current_checksum.len().min(8)]
                ));
            }
        }
    }

    Ok(())
}
