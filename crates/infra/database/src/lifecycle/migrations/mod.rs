//! Extension migration runner backed by the `extension_migrations`
//! bookkeeping table. [`MigrationService`] applies, reverts, and inspects
//! per-extension migration history; reverts live in [`down`], status/plan
//! queries in [`status`], fresh-install baseline stamping in [`stamp`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod down;
mod exec;
mod mark_applied;
mod repair;
mod stamp;
mod status;
mod verify;

pub use mark_applied::MarkAppliedOutcome;
pub use repair::RepairResult;
pub use stamp::FreshnessCheck;
pub use status::{
    AppliedMigration, ChecksumDrift, ExtensionMigrationStatus, MigrationResult, MigrationStatus,
    OrphanedMigration, PendingMigration, SlotCollision, TombstonedSlot,
};

use crate::services::{DatabaseProvider, SqlExecutor};
use exec::{TrackingWrite, check_cross_extension_alters, execute_statements_transactional};
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use systemprompt_identifiers::ToDbValue;
use tracing::{debug, info, warn};

const RECORD_MIGRATION_SQL: &str = "INSERT INTO extension_migrations (id, extension_id, version, \
                                    name, checksum) VALUES ($1, $2, $3, $4, $5)";

#[derive(Debug, Default, Clone, Copy)]
pub struct MigrationConfig {
    pub allow_checksum_drift: bool,
}

pub struct MigrationService<'a> {
    db: &'a dyn DatabaseProvider,
    config: MigrationConfig,
}

impl std::fmt::Debug for MigrationService<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationService")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<'a> MigrationService<'a> {
    pub fn new(db: &'a dyn DatabaseProvider) -> Self {
        Self {
            db,
            config: MigrationConfig::default(),
        }
    }

    #[must_use]
    pub const fn with_config(mut self, config: MigrationConfig) -> Self {
        self.config = config;
        self
    }

    async fn ensure_migrations_table_exists(&self) -> Result<(), LoaderError> {
        let sql = include_str!("../../../schema/extension_migrations.sql");
        SqlExecutor::execute_statements_parsed(self.db, sql)
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: "database".to_owned(),
                message: format!("Failed to ensure migrations table exists: {e}"),
            })
    }

    pub async fn get_applied_migrations(
        &self,
        extension_id: &str,
    ) -> Result<Vec<AppliedMigration>, LoaderError> {
        let result = self
            .db
            .query_raw_with(
                &"SELECT extension_id, version, name, checksum, applied_at FROM \
                  extension_migrations WHERE extension_id = $1 ORDER BY version",
                &[&extension_id],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: extension_id.to_owned(),
                message: format!("Failed to query applied migrations: {e}"),
            })?;

        let migrations = result
            .rows
            .iter()
            .filter_map(|row| {
                Some(AppliedMigration {
                    extension_id: row.get("extension_id")?.as_str()?.to_owned(),
                    version: row.get("version")?.as_i64()? as u32,
                    name: row.get("name")?.as_str()?.to_owned(),
                    checksum: row.get("checksum")?.as_str()?.to_owned(),
                    applied_at: row
                        .get("applied_at")
                        .and_then(|v| v.as_str().map(String::from)),
                })
            })
            .collect();

        Ok(migrations)
    }

    pub async fn run_pending_migrations(
        &self,
        extension: &dyn Extension,
    ) -> Result<MigrationResult, LoaderError> {
        let ext_id = extension.metadata().id;
        let migrations = extension.migrations();

        if migrations.is_empty() {
            return Ok(MigrationResult::default());
        }

        self.ensure_migrations_table_exists().await?;

        let applied = self.get_applied_migrations(ext_id).await?;
        let applied_rows: std::collections::HashMap<u32, &AppliedMigration> =
            applied.iter().map(|m| (m.version, m)).collect();

        warn_orphaned_versions(ext_id, &applied, &migrations);

        let mut migrations_run = 0;
        let mut migrations_skipped = 0;

        for migration in &migrations {
            let row = applied_rows.get(&migration.version).copied();

            if migration.tombstone {
                // Why: a tombstone's name labels the retirement ("retired_chain"),
                // it is not the name of the migration that once held the slot, so
                // comparing it to a tracked row is meaningless — and it failed on
                // exactly the population tombstones exist for. Every established
                // database carries the real names in a retired range, so slot
                // identity was checked against a label and refused the boot.
                debug!(
                    extension = %ext_id,
                    version = migration.version,
                    name = %migration.name,
                    tracked = row.is_some(),
                    "Migration slot is tombstoned, nothing to run"
                );
                continue;
            }

            if let Some(row) = row {
                self.verify_slot_identity(ext_id, migration, Some(row))?;
                self.verify_checksum(ext_id, migration, Some(row.checksum.as_str()))?;
                migrations_skipped += 1;
                debug!(
                    extension = %ext_id,
                    version = migration.version,
                    "Migration already applied, skipping"
                );
                continue;
            }

            self.execute_migration(extension, migration).await?;
            migrations_run += 1;
        }

        if migrations_run > 0 {
            info!(
                extension = %ext_id,
                migrations_run,
                migrations_skipped,
                "Migrations completed"
            );
        }

        Ok(MigrationResult {
            migrations_run,
            migrations_skipped,
        })
    }

    // Why: the recorded name is the only thing that distinguishes a migration
    // edited in place from a slot whose file was deleted and its number reused.
    // The checksum cannot tell them apart — it hashes the SQL alone.
    async fn execute_migration(
        &self,
        extension: &dyn Extension,
        migration: &Migration,
    ) -> Result<(), LoaderError> {
        let ext_id = extension.metadata().id;

        check_cross_extension_alters(extension, migration)?;

        info!(
            extension = %ext_id,
            version = migration.version,
            name = %migration.name,
            no_transaction = migration.no_transaction,
            "Running migration"
        );

        let id = format!("{}_{:03}", ext_id, migration.version);
        let checksum = migration.checksum();
        let record_params: [&dyn ToDbValue; 5] =
            [&id, &ext_id, &migration.version, &migration.name, &checksum];

        if migration.no_transaction {
            SqlExecutor::execute_statements_parsed(self.db, migration.sql)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to execute migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                })?;
            self.db
                .execute(&RECORD_MIGRATION_SQL, &record_params)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to record migration: {e}"),
                })?;
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
                    sql: RECORD_MIGRATION_SQL,
                    params: &record_params,
                }),
            )
            .await?;
        }

        Ok(())
    }
}

// Why: reported, never fatal. Databases predating tombstones carry rows for
// every migration since deleted, and refusing to boot on those would strand
// every established install. Adding the matching `.tombstone` file clears the
// warning; `infra db migrate-status` lists the rows.
pub(crate) fn orphaned_versions(applied: &[AppliedMigration], defined: &[Migration]) -> Vec<u32> {
    let declared: HashSet<u32> = defined.iter().map(|m| m.version).collect();
    applied
        .iter()
        .map(|m| m.version)
        .filter(|version| !declared.contains(version))
        .collect()
}

fn warn_orphaned_versions(ext_id: &str, applied: &[AppliedMigration], defined: &[Migration]) {
    let orphaned = orphaned_versions(applied, defined);
    if orphaned.is_empty() {
        return;
    }
    warn!(
        extension = %ext_id,
        versions = ?orphaned,
        "Applied migrations are no longer declared by the extension; their files were deleted \
         without leaving a tombstone, so the numbers look free but are spent"
    );
}
