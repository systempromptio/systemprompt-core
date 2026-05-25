use anyhow::{Result, anyhow};
use systemprompt_database::MigrationService;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::admin_migrate::select_extensions;
use super::types::{MigrateStatusOutput, MigrateStatusRow, MigrationDriftInfo};

pub(crate) async fn execute_migrate_status(
    ctx: &AppContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_status(db.as_ref(), registry, extension, json, config).await
}

pub(crate) async fn execute_migrate_status_standalone(
    db_ctx: &DatabaseContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = db_ctx.db_pool();
    let registry = ExtensionRegistry::discover()?;
    run_migrate_status(db.as_ref(), &registry, extension, json, config).await
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
    let output = collect_status(&extensions, &migration_service).await?;

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
        render_status_text(&output);
    }

    Ok(())
}

async fn collect_status(
    extensions: &[std::sync::Arc<dyn systemprompt_extension::Extension>],
    migration_service: &MigrationService<'_>,
) -> Result<MigrateStatusOutput> {
    let mut rows: Vec<MigrateStatusRow> = Vec::new();
    let mut drift_rows: Vec<MigrationDriftInfo> = Vec::new();
    let mut total_applied = 0usize;
    let mut total_pending = 0usize;

    for ext in extensions {
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
    Ok(MigrateStatusOutput {
        rows,
        drift: drift_rows,
        total_applied,
        total_pending,
        total_drift,
    })
}

fn render_status_text(output: &MigrateStatusOutput) {
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
