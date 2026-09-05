//! `db migrate-status` subcommand.
//!
//! Reports applied, pending, and checksum-drifted migrations per extension,
//! rendering either a JSON table or a formatted text summary. Provides both the
//! full-context and standalone (`DatabaseContext`-only) entry points.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{ExtensionMigrationStatus, MigrationService};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::cli_settings::CliConfig;
use crate::shared::{CommandOutput, render_result};

use super::admin_migrate::select_extensions;
use super::types::{
    MigrateStatusOutput, MigrateStatusRow, MigrationCollisionInfo, MigrationDriftInfo,
};

pub(super) async fn execute_migrate_status(
    ctx: &AppContext,
    extension: Option<&str>,
    json: bool,
    config: &CliConfig,
) -> Result<()> {
    let db = ctx.db_pool();
    let registry = ctx.extension_registry();
    run_migrate_status(db.as_ref(), registry, extension, json, config).await
}

pub(super) async fn execute_migrate_status_standalone(
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
        let result = CommandOutput::table_of(
            vec!["extension_id", "version", "name", "status", "applied_at"],
            &output.rows,
        )
        .with_title("Migration Status");
        render_result(&result, config);
    } else {
        render_status_text(&output);
    }

    Ok(())
}

// Why: one label per applied slot, most specific first. A tracked tombstone is
// a spent slot, a collision is a reused one, an orphan has no file on disk,
// drift is an edited file; "applied" only when none of those hold.
fn status_label(status: &ExtensionMigrationStatus, version: u32) -> &'static str {
    if status.tombstoned.iter().any(|t| t.version == version) {
        "tombstone"
    } else if status.slot_collisions.iter().any(|c| c.version == version) {
        "collision"
    } else if status.orphaned.iter().any(|o| o.version == version) {
        "orphaned"
    } else if status.drift.iter().any(|d| d.version == version) {
        "drift"
    } else {
        "applied"
    }
}

fn rows_for(status: &ExtensionMigrationStatus) -> Vec<MigrateStatusRow> {
    let row =
        |version: u32, name: &str, label: &str, applied_at: Option<String>| MigrateStatusRow {
            extension_id: status.extension_id.clone(),
            version,
            name: name.to_owned(),
            status: label.to_owned(),
            applied_at,
        };
    let applied = status.applied.iter().map(|a| {
        row(
            a.version,
            &a.name,
            status_label(status, a.version),
            a.applied_at.clone(),
        )
    });
    let untracked_tombstones = status
        .tombstoned
        .iter()
        .filter(|t| !t.tracked)
        .map(|t| row(t.version, &t.name, "tombstone", None));
    let pending = status
        .pending
        .iter()
        .map(|p| row(p.version, &p.name, "pending", None));
    applied.chain(untracked_tombstones).chain(pending).collect()
}

async fn collect_status(
    extensions: &[std::sync::Arc<dyn systemprompt_extension::Extension>],
    migration_service: &MigrationService<'_>,
) -> Result<MigrateStatusOutput> {
    let mut rows: Vec<MigrateStatusRow> = Vec::new();
    let mut drift_rows: Vec<MigrationDriftInfo> = Vec::new();
    let mut collision_rows: Vec<MigrationCollisionInfo> = Vec::new();
    let mut total_applied = 0usize;
    let mut total_pending = 0usize;
    let mut total_orphaned = 0usize;

    for ext in extensions {
        let status = migration_service
            .status(ext.as_ref())
            .await
            .map_err(|e| anyhow!("Failed to get migration status: {}", e))?;
        rows.extend(rows_for(&status));
        total_applied += status.applied.len();
        total_pending += status.pending.len();
        total_orphaned += status.orphaned.len();
        collision_rows.extend(
            status
                .slot_collisions
                .into_iter()
                .map(|c| MigrationCollisionInfo {
                    extension_id: c.extension_id,
                    version: c.version,
                    stored_name: c.stored_name,
                    current_name: c.current_name,
                }),
        );
        drift_rows.extend(status.drift.into_iter().map(|d| MigrationDriftInfo {
            extension_id: d.extension_id,
            version: d.version,
            name: d.name,
            stored_checksum: d.stored_checksum,
            current_checksum: d.current_checksum,
        }));
    }

    rows.sort_by(|a, b| {
        a.extension_id
            .cmp(&b.extension_id)
            .then(a.version.cmp(&b.version))
    });

    let total_drift = drift_rows.len();
    let total_collisions = collision_rows.len();
    Ok(MigrateStatusOutput {
        rows,
        drift: drift_rows,
        collisions: collision_rows,
        total_applied,
        total_pending,
        total_drift,
        total_collisions,
        total_orphaned,
    })
}

fn render_status_text(output: &MigrateStatusOutput) {
    CliService::info(&format!(
        "Applied: {} | Pending: {} | Drift: {} | Collisions: {} | Orphaned: {}",
        output.total_applied,
        output.total_pending,
        output.total_drift,
        output.total_collisions,
        output.total_orphaned
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

    if !output.collisions.is_empty() {
        CliService::info("");
        CliService::warning(&format!(
            "{} migration slot(s) were reused — the recorded row and the file on disk are \
             different migrations. This is not drift and must not be repaired; renumber the new \
             file above every used slot and leave a `NNN_<name>.tombstone` behind.",
            output.total_collisions
        ));
        for c in &output.collisions {
            CliService::info(&format!(
                "  {} v{:03}: recorded='{}' file='{}'",
                c.extension_id, c.version, c.stored_name, c.current_name
            ));
        }
    }

    if output.total_orphaned > 0 {
        CliService::info("");
        CliService::warning(&format!(
            "{} applied migration(s) are no longer declared by their extension. The files were \
             deleted without leaving a tombstone, so those numbers look free but are spent. Add a \
             `NNN_<name>.tombstone` beside the remaining migrations to record each one.",
            output.total_orphaned
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
