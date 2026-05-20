//! Extension migration runner backed by the `extension_migrations`
//! bookkeeping table. [`MigrationService`] applies, reverts, inspects, and
//! squashes per-extension migration history; reverts live in [`down`],
//! status/plan queries in [`status`], squash in [`squash`].

mod down;
mod exec;
mod mark_applied;
mod repair;
mod squash;
mod status;

pub use mark_applied::MarkAppliedOutcome;
pub use repair::RepairResult;
pub use squash::SquashPlan;
pub use status::{
    AppliedMigration, ChecksumDrift, ExtensionMigrationStatus, MigrationResult, MigrationStatus,
    PendingMigration,
};

use crate::services::{DatabaseProvider, SqlExecutor};
use exec::{check_cross_extension_alters, execute_statements_transactional};
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::{debug, info, warn};

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
                extension: "database".to_string(),
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
        let applied_versions: HashSet<u32> = applied.iter().map(|m| m.version).collect();
        let applied_checksums: std::collections::HashMap<u32, &str> = applied
            .iter()
            .map(|m| (m.version, m.checksum.as_str()))
            .collect();

        let mut migrations_run = 0;
        let mut migrations_skipped = 0;

        for migration in &migrations {
            if applied_versions.contains(&migration.version) {
                self.verify_checksum(
                    ext_id,
                    migration,
                    applied_checksums.get(&migration.version).copied(),
                )?;
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

    fn verify_checksum(
        &self,
        ext_id: &str,
        migration: &Migration,
        stored: Option<&str>,
    ) -> Result<(), LoaderError> {
        let Some(stored_checksum) = stored else {
            return Ok(());
        };
        let current_checksum = migration.checksum();
        if stored_checksum == current_checksum {
            return Ok(());
        }
        if self.config.allow_checksum_drift {
            warn!(
                extension = %ext_id,
                version = migration.version,
                name = %migration.name,
                stored_checksum = %stored_checksum,
                current_checksum = %current_checksum,
                "Migration checksum mismatch tolerated by --allow-checksum-drift"
            );
            return Ok(());
        }
        Err(LoaderError::MigrationFailed {
            extension: ext_id.to_string(),
            message: format!(
                "Migration {ver} ('{name}') has been edited since it was applied (stored checksum \
                 {stored_checksum}, current {current_checksum}). Refusing to proceed. Run \
                 `systemprompt infra db migrate-repair --apply` to reconcile the tracking table, \
                 or pass --allow-checksum-drift to bypass the check without fixing it.",
                ver = migration.version,
                name = migration.name,
            ),
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

        if migration.no_transaction {
            SqlExecutor::execute_statements_parsed(self.db, migration.sql)
                .await
                .map_err(|e| LoaderError::MigrationFailed {
                    extension: ext_id.to_string(),
                    message: format!(
                        "Failed to execute migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                })?;
        } else {
            let statements = SqlExecutor::parse_sql_statements(migration.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_string(),
                    message: format!(
                        "Failed to parse migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                }
            })?;
            execute_statements_transactional(self.db, &statements, ext_id, migration).await?;
        }

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

        self.db
            .execute(
                &"INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
                  VALUES ($1, $2, $3, $4, $5)",
                &[&id, &ext_id, &migration.version, &migration.name, &checksum],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!("Failed to record migration: {e}"),
            })?;

        Ok(())
    }
}
