use anyhow::{Result, anyhow};
use systemprompt_database::MigrationService;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::admin_migrate::select_extensions;
use super::types::{MigratePlanOutput, PendingMigrationInfo};

pub(crate) async fn execute_migrate_plan(
    ctx: &AppContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_plan(db.as_ref(), registry, extension, json, config).await
}

pub(crate) async fn execute_migrate_plan_standalone(
    db_ctx: &DatabaseContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover()?;
    run_migrate_plan(db.as_ref(), &registry, extension, json, config).await
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
