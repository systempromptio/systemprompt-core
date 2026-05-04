//! Extension migration runner backed by the `extension_migrations`
//! bookkeeping table.

use crate::services::{DatabaseProvider, SqlExecutor};
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::{debug, info, warn};

/// Row in the `extension_migrations` table — a migration that has already
/// been recorded as applied.
#[derive(Debug, Clone)]
pub struct AppliedMigration {
    /// Owning extension id.
    pub extension_id: String,
    /// Migration version number (extension-local, monotonically increasing).
    pub version: u32,
    /// Human-readable migration name.
    pub name: String,
    /// SHA checksum captured at apply time, used for drift detection.
    pub checksum: String,
}

/// Outcome of [`MigrationService::run_pending_migrations`].
#[derive(Debug, Default, Clone, Copy)]
pub struct MigrationResult {
    /// Number of migrations that were just applied.
    pub migrations_run: usize,
    /// Number of migrations that were already applied and skipped.
    pub migrations_skipped: usize,
}

/// Stateless service that orchestrates extension migrations against `db`.
pub struct MigrationService<'a> {
    db: &'a dyn DatabaseProvider,
}

impl std::fmt::Debug for MigrationService<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationService").finish_non_exhaustive()
    }
}

impl<'a> MigrationService<'a> {
    /// Construct a service bound to `db`.
    pub fn new(db: &'a dyn DatabaseProvider) -> Self {
        Self { db }
    }

    async fn ensure_migrations_table_exists(&self) -> Result<(), LoaderError> {
        let sql = include_str!("../../schema/extension_migrations.sql");
        SqlExecutor::execute_statements_parsed(self.db, sql)
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: "database".to_string(),
                message: format!("Failed to ensure migrations table exists: {e}"),
            })
    }

    /// Return every migration row for `extension_id`, ordered by version.
    pub async fn get_applied_migrations(
        &self,
        extension_id: &str,
    ) -> Result<Vec<AppliedMigration>, LoaderError> {
        let result = self
            .db
            .query_raw_with(
                &"SELECT extension_id, version, name, checksum FROM extension_migrations WHERE \
                  extension_id = $1 ORDER BY version",
                vec![serde_json::Value::String(extension_id.to_string())],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: extension_id.to_string(),
                message: format!("Failed to query applied migrations: {e}"),
            })?;

        let migrations = result
            .rows
            .iter()
            .filter_map(|row| {
                Some(AppliedMigration {
                    extension_id: row.get("extension_id")?.as_str()?.to_string(),
                    version: row.get("version")?.as_i64()? as u32,
                    name: row.get("name")?.as_str()?.to_string(),
                    checksum: row.get("checksum")?.as_str()?.to_string(),
                })
            })
            .collect();

        Ok(migrations)
    }

    /// Apply every migration for `extension` whose version has not yet been
    /// recorded in `extension_migrations`. Logs (but does not fail) on
    /// checksum drift between the recorded migration and the on-disk SQL.
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
        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();
        let applied_checksums: std::collections::HashMap<u32, &str> = applied
            .iter()
            .map(|m| (m.version, m.checksum.as_str()))
            .collect();

        let mut migrations_run = 0;
        let mut migrations_skipped = 0;

        for migration in &migrations {
            if applied_versions.contains(&migration.version) {
                let current_checksum = migration.checksum();
                if let Some(&stored_checksum) = applied_checksums.get(&migration.version) {
                    if stored_checksum != current_checksum {
                        warn!(
                            extension = %ext_id,
                            version = migration.version,
                            name = %migration.name,
                            stored_checksum = %stored_checksum,
                            current_checksum = %current_checksum,
                            "Migration checksum mismatch - SQL has changed since it was applied"
                        );
                    }
                }
                migrations_skipped += 1;
                debug!(
                    extension = %ext_id,
                    version = migration.version,
                    "Migration already applied, skipping"
                );
                continue;
            }

            self.execute_migration(ext_id, migration).await?;
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
        ext_id: &str,
        migration: &Migration,
    ) -> Result<(), LoaderError> {
        info!(
            extension = %ext_id,
            version = migration.version,
            name = %migration.name,
            "Running migration"
        );

        SqlExecutor::execute_statements_parsed(self.db, migration.sql)
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!(
                    "Failed to execute migration {} ({}): {e}",
                    migration.version, migration.name
                ),
            })?;

        self.record_migration(ext_id, migration).await?;

        Ok(())
    }

    async fn record_migration(
        &self,
        ext_id: &str,
        migration: &Migration,
    ) -> Result<(), LoaderError> {
        let id = format!("{}_{:03}", ext_id, migration.version);
        let checksum = migration.checksum();
        let name = migration.name.replace('\'', "''");

        let sql = format!(
            "INSERT INTO extension_migrations (id, extension_id, version, name, checksum) VALUES \
             ('{}', '{}', {}, '{}', '{}')",
            id, ext_id, migration.version, name, checksum
        );

        self.db
            .execute_raw(&sql)
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!("Failed to record migration: {e}"),
            })?;

        Ok(())
    }

    /// Return defined / applied / pending counts for `extension` without
    /// running anything.
    pub async fn get_migration_status(
        &self,
        extension: &dyn Extension,
    ) -> Result<MigrationStatus, LoaderError> {
        self.ensure_migrations_table_exists().await?;

        let ext_id = extension.metadata().id;
        let defined_migrations = extension.migrations();
        let applied = self.get_applied_migrations(ext_id).await?;

        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();

        let pending: Vec<_> = defined_migrations
            .iter()
            .filter(|m| !applied_versions.contains(&m.version))
            .cloned()
            .collect();

        Ok(MigrationStatus {
            extension_id: ext_id.to_string(),
            total_defined: defined_migrations.len(),
            total_applied: applied.len(),
            pending_count: pending.len(),
            pending,
            applied,
        })
    }
}

/// Snapshot returned by [`MigrationService::get_migration_status`].
#[derive(Debug)]
pub struct MigrationStatus {
    /// Owning extension id.
    pub extension_id: String,
    /// Number of migrations defined in code.
    pub total_defined: usize,
    /// Number of migrations recorded as applied in the database.
    pub total_applied: usize,
    /// Number of defined migrations that have not yet been applied.
    pub pending_count: usize,
    /// Pending migrations in version order.
    pub pending: Vec<Migration>,
    /// Already-applied migrations in version order.
    pub applied: Vec<AppliedMigration>,
}
