//! Extension migration runner backed by the `extension_migrations`
//! bookkeeping table. [`MigrationService`] applies, reverts, and inspects
//! per-extension migration history; reverts live in [`down`], status/plan
//! queries in [`status`], fresh-install baseline stamping in [`stamp`] (whose
//! rows the installer commits with the structural DDL they describe).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod budget;
mod checksum_transition;
mod down;
mod exec;
mod mark_applied;
mod repair;
mod stamp;
mod status;
mod verify;

pub use mark_applied::MarkAppliedOutcome;
pub use repair::RepairResult;
pub use stamp::{BaselineStamp, FreshnessCheck, is_retirement};
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

pub(crate) const RECORD_MIGRATION_SQL: &str = "INSERT INTO extension_migrations (id, extension_id, version, \
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

        result
            .rows
            .iter()
            .map(|row| decode_applied_row(extension_id, row))
            .collect()
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
        self.transition_checksums(ext_id, &migrations, &applied)
            .await?;
        let applied_rows: std::collections::HashMap<u32, &AppliedMigration> =
            applied.iter().map(|m| (m.version, m)).collect();

        warn_orphaned_versions(ext_id, &applied, &migrations);

        let mut migrations_run = 0;
        let mut migrations_skipped = 0;

        for migration in &migrations {
            let row = applied_rows.get(&migration.version).copied();

            if migration.tombstone {
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
                self.verify_checksum(ext_id, migration, &row.checksum)?;
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
            // Why: no transaction to scope the bound to, so it is set on the
            // connection and reset after — `SET LOCAL` would be a silent
            // no-op here, leaving this path the only unbounded one.
            self.apply_timeouts(ext_id, migration).await?;
            let outcome = SqlExecutor::execute_statements_parsed(self.db, migration.sql)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to execute migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                });
            self.clear_timeouts(ext_id).await?;
            outcome?;
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

    async fn apply_timeouts(&self, ext_id: &str, migration: &Migration) -> Result<(), LoaderError> {
        let timeout = budget::statement_timeout(migration);
        self.set_timeouts(ext_id, &budget::timeout_statements(timeout, false))
            .await
    }

    // Why: the connection outlives this migration, so a bound left on it
    // would apply to whatever ran next — including the application's own
    // queries if the pool hands the connection back.
    async fn clear_timeouts(&self, ext_id: &str) -> Result<(), LoaderError> {
        self.set_timeouts(
            ext_id,
            &[
                "SET statement_timeout = DEFAULT".to_owned(),
                "SET lock_timeout = DEFAULT".to_owned(),
            ],
        )
        .await
    }

    async fn set_timeouts(&self, ext_id: &str, statements: &[String]) -> Result<(), LoaderError> {
        for statement in statements {
            self.db
                .execute(&statement.as_str(), &[])
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to run `{statement}`: {e}"),
                })?;
        }
        Ok(())
    }
}

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

fn decode_applied_row(
    extension_id: &str,
    row: &crate::models::JsonRow,
) -> Result<AppliedMigration, LoaderError> {
    let malformed = |column: &str| LoaderError::MigrationFailed {
        extension: extension_id.to_owned(),
        message: format!("extension_migrations row has a malformed `{column}` column"),
    };
    let text = |column: &str| -> Result<String, LoaderError> {
        row.get(column)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| malformed(column))
    };
    let version = row
        .get("version")
        .and_then(serde_json::Value::as_i64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| malformed("version"))?;
    let checksum = text("checksum")?;
    Ok(AppliedMigration {
        extension_id: text("extension_id")?,
        version,
        name: text("name")?,
        checksum,
        applied_at: row
            .get("applied_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}
