use crate::services::{DatabaseProvider, SqlExecutor};
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct AppliedMigration {
    pub extension_id: String,
    pub version: u32,
    pub name: String,
    pub checksum: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MigrationResult {
    pub migrations_run: usize,
    pub migrations_skipped: usize,
}

pub struct MigrationService<'a> {
    db: &'a dyn DatabaseProvider,
}

impl std::fmt::Debug for MigrationService<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationService").finish_non_exhaustive()
    }
}

impl<'a> MigrationService<'a> {
    pub fn new(db: &'a dyn DatabaseProvider) -> Self {
        Self { db }
    }

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

    pub async fn run_pending_migrations(
        &self,
        extension: &dyn Extension,
    ) -> Result<MigrationResult, LoaderError> {
        let ext_id = extension.metadata().id;
        let migrations = extension.migrations();

        if migrations.is_empty() {
            return Ok(MigrationResult::default());
        }

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

    pub async fn get_migration_status(
        &self,
        extension: &dyn Extension,
    ) -> Result<MigrationStatus, LoaderError> {
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

#[derive(Debug)]
pub struct MigrationStatus {
    pub extension_id: String,
    pub total_defined: usize,
    pub total_applied: usize,
    pub pending_count: usize,
    pub pending: Vec<Migration>,
    pub applied: Vec<AppliedMigration>,
}
