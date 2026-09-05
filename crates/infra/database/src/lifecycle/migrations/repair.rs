//! Migration checksum-drift repair.
//!
//! When an already-applied migration file is edited in place, its stored
//! checksum stops matching the file and the runner refuses to proceed.
//! [`MigrationService::repair_drift`] re-executes each drifted migration and
//! rewrites its stored checksum in the same transaction; the tracking row is
//! never deleted, so a failed re-apply rolls back to "drifted but tracked"
//! instead of leaving the migration untracked and crash-looping the next
//! boot. Re-applying requires the migration SQL to be re-executable against
//! the current schema — a later migration may have invalidated that, in which
//! case [`MigrationService::reconcile_drift`] rewrites the stored checksum
//! without executing any SQL. `no_transaction` migrations cannot be repaired
//! atomically: a mid-SQL failure leaves the row tracked with the old
//! checksum, which still reports as drift rather than crash-looping.
//!
//! Both entry points refuse outright when the recorded row for a slot names a
//! different migration than the file now occupying it. That is a reused slot,
//! not drift, and reconciling it would stamp one migration's checksum onto a
//! row describing another — silencing that row's drift detector for good.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::exec::{TrackingWrite, check_cross_extension_alters, execute_statements_transactional};
use super::{ChecksumDrift, ExtensionMigrationStatus, MigrationService};
use crate::lifecycle::installation::BootstrapLockGuard;
use crate::services::SqlExecutor;
use systemprompt_extension::{Extension, LoaderError, Migration};
use systemprompt_identifiers::ToDbValue;

const UPDATE_CHECKSUM_SQL: &str =
    "UPDATE extension_migrations SET checksum = $3 WHERE extension_id = $1 AND version = $2";

#[derive(Debug, Default, Clone)]
pub struct RepairResult {
    pub repaired: Vec<ChecksumDrift>,
    // Why: drifted migrations whose SQL was actually re-executed. Zero for
    // reconcile_drift, which only rewrites bookkeeping.
    pub reapplied: usize,
    // Why: previously-unapplied migrations run as part of the repair — a
    // different number, and reporting it as re-applied is what hid the bug.
    pub migrations_run: usize,
}

impl MigrationService<'_> {
    pub async fn repair_drift(
        &self,
        extension: &dyn Extension,
    ) -> Result<RepairResult, LoaderError> {
        let status = self.status(extension).await?;
        Self::refuse_slot_collisions(&status)?;

        if status.drift.is_empty() {
            return Ok(RepairResult::default());
        }

        let reapplied = status.drift.len();
        let guard = BootstrapLockGuard::acquire(self.db).await?;
        let outcome = self.reapply_drifted(extension, &status.drift).await;
        let pending = match outcome {
            Ok(()) => self.run_pending_migrations(extension).await,
            Err(e) => Err(e),
        };
        guard.release().await;
        let result = pending?;

        Ok(RepairResult {
            repaired: status.drift,
            reapplied,
            migrations_run: result.migrations_run,
        })
    }

    pub async fn reconcile_drift(
        &self,
        extension: &dyn Extension,
    ) -> Result<RepairResult, LoaderError> {
        let status = self.status(extension).await?;
        Self::refuse_slot_collisions(&status)?;

        if status.drift.is_empty() {
            return Ok(RepairResult::default());
        }

        let guard = BootstrapLockGuard::acquire(self.db).await?;
        let mut outcome = Ok(());
        for drift in &status.drift {
            if let Err(e) = self.rewrite_checksum(drift).await {
                outcome = Err(e);
                break;
            }
        }
        guard.release().await;
        outcome?;

        Ok(RepairResult {
            repaired: status.drift,
            reapplied: 0,
            migrations_run: 0,
        })
    }

    // Why: matching a recorded row on (extension_id, version) alone cannot
    // tell an edited migration from a reused slot. Reconciling a collision
    // stamps one migration's checksum onto a row describing another, which
    // silences that row's drift detector permanently. Refuse instead.
    fn refuse_slot_collisions(status: &ExtensionMigrationStatus) -> Result<(), LoaderError> {
        let Some(collision) = status.slot_collisions.first() else {
            return Ok(());
        };
        Err(LoaderError::MigrationSlotReused {
            extension: collision.extension_id.clone(),
            version: collision.version,
            stored_name: collision.stored_name.clone(),
            current_name: collision.current_name.clone(),
        })
    }

    async fn rewrite_checksum(&self, drift: &ChecksumDrift) -> Result<(), LoaderError> {
        self.db
            .execute(
                &UPDATE_CHECKSUM_SQL,
                &[&drift.extension_id, &drift.version, &drift.current_checksum],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: drift.extension_id.clone(),
                message: format!(
                    "Failed to rewrite checksum for migration {} ('{}'): {e}",
                    drift.version, drift.name
                ),
            })?;
        Ok(())
    }

    async fn reapply_drifted(
        &self,
        extension: &dyn Extension,
        drift: &[ChecksumDrift],
    ) -> Result<(), LoaderError> {
        let ext_id = extension.metadata().id;
        let migrations = extension.migrations();

        for d in drift {
            let migration = migrations
                .iter()
                .find(|m| m.version == d.version)
                .ok_or_else(|| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Drifted migration {} ('{}') is no longer declared by extension \
                         '{ext_id}'",
                        d.version, d.name
                    ),
                })?;
            self.reapply_one(extension, migration, d).await?;
        }

        Ok(())
    }

    async fn reapply_one(
        &self,
        extension: &dyn Extension,
        migration: &Migration,
        drift: &ChecksumDrift,
    ) -> Result<(), LoaderError> {
        let ext_id = extension.metadata().id;

        check_cross_extension_alters(extension, migration)?;

        tracing::info!(
            extension = %ext_id,
            version = migration.version,
            name = %migration.name,
            no_transaction = migration.no_transaction,
            "Re-applying drifted migration"
        );

        let update_params: [&dyn ToDbValue; 3] =
            [&drift.extension_id, &drift.version, &drift.current_checksum];

        if migration.no_transaction {
            SqlExecutor::execute_statements_parsed(self.db, migration.sql)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to re-apply drifted migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                })?;
            self.rewrite_checksum(drift).await
        } else {
            let statements = SqlExecutor::parse_sql_statements(migration.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to parse migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                }
            })?;
            execute_statements_transactional(
                self.db,
                &statements,
                ext_id,
                migration,
                Some(TrackingWrite {
                    sql: UPDATE_CHECKSUM_SQL,
                    params: &update_params,
                }),
            )
            .await
        }
    }
}
