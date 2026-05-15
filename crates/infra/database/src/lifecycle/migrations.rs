//! Extension migration runner backed by the `extension_migrations`
//! bookkeeping table.

use crate::services::{DatabaseProvider, SqlExecutor};
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};
use tracing::{debug, info, warn};

/// Inspect a migration's SQL with `pg_query` and return every table name that
/// appears as the target of an `ALTER TABLE` statement. Returns parser errors
/// as `Err` so callers can attribute them to the originating extension /
/// migration.
fn alter_table_targets(sql: &str) -> Result<Vec<String>, String> {
    let parsed = pg_query::parse(sql).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = Vec::new();
    for stmt in parsed.protobuf.stmts {
        let Some(node) = stmt.stmt.and_then(|s| s.node) else {
            continue;
        };
        if let pg_query::NodeEnum::AlterTableStmt(alter) = node {
            if let Some(rv) = alter.relation {
                out.push(rv.relname);
            }
        }
    }
    Ok(out)
}

/// Per-statement counterpart of
/// [`crate::lifecycle::installation::extension::execute_statements_transactional`],
/// scoped to a single migration. Lives here so the migration runner can apply
/// a `BEGIN; … COMMIT` envelope around an already-parsed statement list and
/// fall back to `ROLLBACK` on any failure — the bookkeeping write is only
/// recorded after the commit succeeds.
async fn execute_statements_transactional(
    db: &dyn DatabaseProvider,
    statements: &[String],
    ext_id: &str,
    migration: &Migration,
) -> Result<(), LoaderError> {
    if statements.is_empty() {
        return Ok(());
    }

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_string(),
            message: format!(
                "Failed to begin transaction for migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        if let Err(e) = tx.execute(&sql_str, &[]).await {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!(
                    "Migration {ver} ({name}) statement {n}/{total} failed: \
                     {e}{rollback_note}\nSQL:\n{statement}",
                    ver = migration.version,
                    name = migration.name,
                    n = idx + 1,
                ),
            });
        }
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_string(),
            message: format!(
                "Failed to commit migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    Ok(())
}

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
        let sql = include_str!("../../schema/extension_migrations.sql");
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
                        if self.config.allow_checksum_drift {
                            warn!(
                                extension = %ext_id,
                                version = migration.version,
                                name = %migration.name,
                                stored_checksum = %stored_checksum,
                                current_checksum = %current_checksum,
                                "Migration checksum mismatch tolerated by --allow-checksum-drift"
                            );
                        } else {
                            return Err(LoaderError::MigrationFailed {
                                extension: ext_id.to_string(),
                                message: format!(
                                    "Migration {ver} ('{name}') has been edited since it was \
                                     applied (stored checksum {stored_checksum}, current \
                                     {current_checksum}). Refusing to proceed. Re-run with \
                                     --allow-checksum-drift to override.",
                                    ver = migration.version,
                                    name = migration.name,
                                ),
                            });
                        }
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

        let altered =
            alter_table_targets(migration.sql).map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!(
                    "Failed to parse migration {} ({}) for cross-extension ALTER check: {e}",
                    migration.version, migration.name
                ),
            })?;

        if !altered.is_empty() {
            let mut allowed: HashSet<&str> = HashSet::new();
            for t in extension.owned_tables() {
                allowed.insert(t);
            }
            for t in extension.cross_extension_tables() {
                allowed.insert(t);
            }
            for table in &altered {
                if !allowed.contains(table.as_str()) {
                    return Err(LoaderError::CrossExtensionAlterUndeclared {
                        extension: ext_id.to_string(),
                        table: table.clone(),
                    });
                }
            }
        }

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

    pub async fn run_down_migrations(
        &self,
        extension: &dyn Extension,
        count: u32,
    ) -> Result<MigrationResult, LoaderError> {
        if count == 0 {
            return Ok(MigrationResult::default());
        }

        let ext_id = extension.metadata().id;
        self.ensure_migrations_table_exists().await?;

        let result = self
            .db
            .query_raw_with(
                &"SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY \
                  version DESC LIMIT $2",
                vec![
                    serde_json::Value::String(ext_id.to_string()),
                    serde_json::Value::Number(serde_json::Number::from(count)),
                ],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!("Failed to query applied migrations for revert: {e}"),
            })?;

        let versions: Vec<u32> = result
            .rows
            .iter()
            .filter_map(|row| row.get("version")?.as_i64().map(|v| v as u32))
            .collect();

        if versions.is_empty() {
            return Ok(MigrationResult::default());
        }

        let migrations = extension.migrations();
        let mut migrations_run = 0;

        for version in versions {
            let migration = migrations
                .iter()
                .find(|m| m.version == version)
                .ok_or_else(|| LoaderError::MigrationFailed {
                    extension: ext_id.to_string(),
                    message: format!(
                        "Cannot revert migration {version}: not declared in \
                         Extension::migrations()"
                    ),
                })?;

            let down_sql = migration
                .down
                .ok_or_else(|| LoaderError::MigrationNotReversible {
                    extension: ext_id.to_string(),
                    version,
                })?;

            info!(
                extension = %ext_id,
                version = migration.version,
                name = %migration.name,
                "Reverting migration"
            );

            let statements = SqlExecutor::parse_sql_statements(down_sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_string(),
                    message: format!(
                        "Failed to parse down migration {} ({}): {e}",
                        migration.version, migration.name
                    ),
                }
            })?;
            execute_statements_transactional(self.db, &statements, ext_id, migration).await?;

            self.delete_migration_record(ext_id, version).await?;
            migrations_run += 1;
        }

        Ok(MigrationResult {
            migrations_run,
            migrations_skipped: 0,
        })
    }

    async fn delete_migration_record(&self, ext_id: &str, version: u32) -> Result<(), LoaderError> {
        self.db
            .query_raw_with(
                &"DELETE FROM extension_migrations WHERE extension_id = $1 AND version = $2",
                vec![
                    serde_json::Value::String(ext_id.to_string()),
                    serde_json::Value::Number(serde_json::Number::from(version)),
                ],
            )
            .await
            .map_err(|e| LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!("Failed to delete migration record {version}: {e}"),
            })?;

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

#[derive(Debug)]
pub struct MigrationStatus {
    pub extension_id: String,
    pub total_defined: usize,
    pub total_applied: usize,
    pub pending_count: usize,
    pub pending: Vec<Migration>,
    pub applied: Vec<AppliedMigration>,
}
