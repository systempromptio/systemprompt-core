//! `db migrate-repair` subcommand.
//!
//! Detects and, with `--apply`, repairs checksum drift between stored migration
//! rows and the current migration sources for one or all extensions. Provides
//! both the full-context and standalone (`DatabaseContext`-only) entry points.
//!
//! `--apply` re-executes the drifted SQL; `--reconcile-only --apply` rewrites
//! the stored checksum and runs nothing — the only path that skips execution.
//! The two report differently, because a bookkeeping-only run asserts the
//! schema matches the file without checking. A reused migration slot is
//! refused on every path, including the read-only report, so the dry run
//! cannot say "nothing to repair" about a slot `--apply` would reject.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use crate::kernel::KernelHost;
use std::sync::Arc;
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::BuiltState;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::DatabaseContext;

use systemprompt_extension::cli::CommandOutput;

use super::admin_migrate::select_extensions;
use super::types::{MigrateRepairOutput, MigrationDriftInfo};

#[derive(Clone, Copy)]
pub(super) struct RepairArgs<'a> {
    pub extension: Option<&'a ExtensionId>,
    pub apply: bool,
    pub reconcile_only: bool,
}

pub(super) async fn execute_migrate_repair(
    host: &dyn KernelHost,
    args: RepairArgs<'_>,
) -> Result<CommandOutput> {
    let sys_config = host.config()?;
    let database = Arc::new(
        Database::connect(
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?
        .with_session_url(
            sys_config.database_session_url.as_deref(),
            &systemprompt_database::PoolConfig::default().for_session(None),
        )
        .await
        .context("Failed to connect to the session database")?,
    );

    run_migrate_repair(database.session(), host.admitted()?.built(), args).await
}

pub(super) async fn execute_migrate_repair_standalone(
    db_ctx: &DatabaseContext,
    built: &BuiltState,
    args: RepairArgs<'_>,
) -> Result<CommandOutput> {
    run_migrate_repair(db_ctx.db_pool().session(), built, args).await
}

async fn run_migrate_repair(
    write_provider: &dyn DatabaseProvider,
    built: &BuiltState,
    args: RepairArgs<'_>,
) -> Result<CommandOutput> {
    let RepairArgs {
        extension,
        apply,
        reconcile_only,
    } = args;
    let extensions = select_extensions(built, extension)?;
    let migration_service = MigrationService::new(write_provider);

    let mut drift_rows: Vec<MigrationDriftInfo> = Vec::new();
    let mut migrations_run = 0usize;
    let mut reapplied = 0usize;

    for ext in &extensions {
        let drift = if apply && reconcile_only {
            let result = migration_service
                .reconcile_drift(ext.as_ref())
                .await
                .context("Failed to reconcile migration checksums")?;
            result.repaired
        } else if apply {
            let result = migration_service
                .repair_drift(ext.as_ref())
                .await
                .context("Failed to repair migrations")?;
            migrations_run += result.migrations_run;
            reapplied += result.reapplied;
            result.repaired
        } else {
            let status = migration_service
                .status(ext.as_ref())
                .await
                .context("Failed to get migration status")?;
            MigrationService::refuse_slot_collisions(&status)
                .context("Cannot repair migrations")?;
            status.drift
        };
        for d in drift {
            drift_rows.push(MigrationDriftInfo {
                extension_id: ExtensionId::new(d.extension_id),
                version: d.version,
                name: d.name,
                stored_checksum: d.stored_checksum,
                current_checksum: d.current_checksum,
            });
        }
    }

    drift_rows.sort_by(|a, b| {
        a.extension_id
            .cmp(&b.extension_id)
            .then(a.version.cmp(&b.version))
    });

    let output = MigrateRepairOutput {
        applied: apply,
        reconcile_only,
        drift: drift_rows,
        reapplied,
        migrations_run,
    };

    Ok(repair_output(&output))
}

fn repair_output(output: &MigrateRepairOutput) -> CommandOutput {
    let card = CommandOutput::card_value("Migration Repair", output);
    if output.drift.is_empty() {
        return card.with_note(NoticeLine::new(
            "success",
            "No checksum drift — nothing to repair.",
        ));
    }

    let header = if output.applied && output.reconcile_only {
        "Reconciled migration(s) (bookkeeping only, no SQL executed):"
    } else if output.applied {
        "Re-executed migration(s):"
    } else {
        "Drifted migration(s):"
    };
    let mut notes = vec![NoticeLine::new("info", header)];
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

    notes.push(if output.applied && output.reconcile_only {
        NoticeLine::new(
            "warning",
            format!(
                "Bookkeeping only: rewrote the stored checksum of {} migration(s). NO SQL WAS \
                 EXECUTED, so the database schema was not checked against the edited files. \
                 Drift: 0",
                output.drift.len()
            ),
        )
    } else if output.applied {
        NoticeLine::new(
            "success",
            format!(
                "Re-executed the SQL of {} drifted migration(s) and rewrote their stored \
                 checksums; {} previously-pending migration(s) also run. Drift: 0",
                output.reapplied, output.migrations_run
            ),
        )
    } else {
        NoticeLine::new(
            "warning",
            format!(
                "{} migration(s) drifted. Re-run with --reconcile-only --apply to rewrite stored \
                 checksums without executing SQL, or --apply to re-execute the edited migrations.",
                output.drift.len()
            ),
        )
    });
    notes.into_iter().fold(card, CommandOutput::with_note)
}
