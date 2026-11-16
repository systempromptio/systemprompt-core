//! `db migrate-status` subcommand.
//!
//! Reports applied, pending, and checksum-drifted migrations per extension,
//! rendering either a JSON table or a formatted text summary. Provides both the
//! full-context and standalone (`DatabaseContext`-only) entry points.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{ExtensionMigrationStatus, MigrationService, SchemaMember};
use systemprompt_extension::BuiltState;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{AppContext, DatabaseContext};

use systemprompt_extension::cli::CommandOutput;

use super::admin_migrate::select_extensions;
use super::types::{
    MigrateStatusOutput, MigrateStatusRow, MigrationCollisionInfo, MigrationDriftInfo,
};

pub(super) async fn execute_migrate_status(
    ctx: &AppContext,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
) -> Result<CommandOutput> {
    run_migrate_status(ctx.db_pool().as_ref(), built, extension).await
}

pub(super) async fn execute_migrate_status_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
) -> Result<CommandOutput> {
    run_migrate_status(db_ctx.db_pool().as_ref(), built, extension).await
}

async fn run_migrate_status(
    db: &dyn DatabaseProvider,
    built: &BuiltState,
    extension: Option<&ExtensionId>,
) -> Result<CommandOutput> {
    let extensions = select_extensions(built, extension)?;
    let migration_service = MigrationService::new(db);
    let output = collect_status(&extensions, &migration_service).await?;

    let table = CommandOutput::table_of(
        vec!["extension_id", "version", "name", "status", "applied_at"],
        &output.rows,
    )
    .with_title("Migration Status");
    Ok(status_notes(&output)
        .into_iter()
        .fold(table, CommandOutput::with_note))
}

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
            extension_id: ExtensionId::new(status.extension_id.clone()),
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
    extensions: &[std::sync::Arc<dyn SchemaMember>],
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
            .context("Failed to get migration status")?;
        rows.extend(rows_for(&status));
        total_applied += status.applied.len();
        total_pending += status.pending.len();
        total_orphaned += status.orphaned.len();
        collision_rows.extend(
            status
                .slot_collisions
                .into_iter()
                .map(|c| MigrationCollisionInfo {
                    extension_id: ExtensionId::new(c.extension_id),
                    version: c.version,
                    stored_name: c.stored_name,
                    current_name: c.current_name,
                }),
        );
        drift_rows.extend(status.drift.into_iter().map(|d| MigrationDriftInfo {
            extension_id: ExtensionId::new(d.extension_id),
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

fn status_notes(output: &MigrateStatusOutput) -> Vec<NoticeLine> {
    let mut notes = vec![NoticeLine::new(
        "info",
        format!(
            "Applied: {} | Pending: {} | Drift: {} | Collisions: {} | Orphaned: {}",
            output.total_applied,
            output.total_pending,
            output.total_drift,
            output.total_collisions,
            output.total_orphaned
        ),
    )];

    if !output.collisions.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} migration slot(s) were reused — the recorded row and the file on disk are \
                 different migrations. This is not drift and must not be repaired; renumber the \
                 new file above every used slot and leave a `NNN_<name>.tombstone` behind.",
                output.total_collisions
            ),
        ));
        notes.extend(output.collisions.iter().map(|c| {
            NoticeLine::new(
                "info",
                format!(
                    "  {} v{:03}: recorded='{}' file='{}'",
                    c.extension_id, c.version, c.stored_name, c.current_name
                ),
            )
        }));
    }

    if output.total_orphaned > 0 {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "{} applied migration(s) are no longer declared by their extension. The files \
                 were deleted without leaving a tombstone, so those numbers look free but are \
                 spent. Add a `NNN_<name>.tombstone` beside the remaining migrations to record \
                 each one.",
                output.total_orphaned
            ),
        ));
    }

    if !output.drift.is_empty() {
        notes.push(NoticeLine::new(
            "warning",
            format!("{} checksum drift(s) detected:", output.total_drift),
        ));
        notes.extend(output.drift.iter().map(|d| {
            NoticeLine::new(
                "info",
                format!(
                    "  {} v{:03} {}: stored={} current={}",
                    d.extension_id,
                    d.version,
                    d.name,
                    &d.stored_checksum[..d.stored_checksum.len().min(8)],
                    &d.current_checksum[..d.current_checksum.len().min(8)]
                ),
            )
        }));
    }
    notes
}
