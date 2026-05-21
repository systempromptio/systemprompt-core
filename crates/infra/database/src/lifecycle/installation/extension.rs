//! Schema installation for compile-time-registered
//! [`systemprompt_extension::Extension`] instances.
//!
//! Installation runs globally in three phases — structural DDL, then
//! migrations, then dependent DDL — so a legacy database reaches its target
//! shape before any `CREATE INDEX`/`VIEW` references a migration-added column.
//! A session-scoped advisory lock serialises concurrent boots. See
//! `instructions/information/migrations.md`.

use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError};
use tracing::{debug, info, warn};

use super::prepare::{PreparedSchema, prepare_extension_schema};
use super::seeds::apply_seeds;
use crate::lifecycle::migrations::{MigrationConfig, MigrationService};
use crate::services::DatabaseProvider;

/// Every `systemprompt` process must lock on this same value for the advisory
/// lock to serialise concurrent boots.
const BOOTSTRAP_ADVISORY_LOCK_KEY: i64 = 0x73_70_72_6F_6D_70_74_01;

pub async fn install_extension_schemas(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    install_extension_schemas_with_config(registry, db, &[]).await
}

pub async fn install_extension_schemas_with_config(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
) -> Result<(), LoaderError> {
    install_extension_schemas_full(
        registry,
        db,
        disabled_extensions,
        MigrationConfig::default(),
    )
    .await
}

pub async fn install_extension_schemas_full(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
    migration_config: MigrationConfig,
) -> Result<(), LoaderError> {
    let schema_extensions = registry.enabled_schema_extensions(disabled_extensions);

    if schema_extensions.is_empty() {
        info!("No extension schemas to install");
        return Ok(());
    }

    info!(
        "Installing schemas for {} extensions",
        schema_extensions.len()
    );

    let guard = BootstrapLockGuard::acquire(db).await?;

    let result = run_install(db, &schema_extensions, migration_config).await;

    guard.release().await;

    result?;

    info!("Extension schema installation complete");
    Ok(())
}

async fn run_install(
    db: &dyn DatabaseProvider,
    schema_extensions: &[std::sync::Arc<dyn Extension>],
    migration_config: MigrationConfig,
) -> Result<(), LoaderError> {
    let migration_service = MigrationService::new(db).with_config(migration_config);

    let mut prepared: Vec<PreparedSchema> = Vec::with_capacity(schema_extensions.len());
    for ext in schema_extensions {
        prepared.push(prepare_extension_schema(ext.as_ref())?);
    }

    validate_table_ownership(&prepared, schema_extensions)?;

    for p in &prepared {
        execute_statements_transactional(db, &p.structural, &p.extension_id).await?;
    }

    for ext in schema_extensions {
        if ext.has_migrations() {
            debug!(extension = %ext.id(), "Running pending migrations");
            migration_service
                .run_pending_migrations(ext.as_ref())
                .await?;
        }
    }

    for p in &prepared {
        execute_statements_transactional(db, &p.dependent, &p.extension_id).await?;
        for cols in &p.columns_to_validate {
            validate_extension_columns(db, cols, &p.extension_id).await?;
        }
    }

    for ext in schema_extensions {
        apply_seeds(ext.as_ref(), db).await?;
    }

    Ok(())
}

/// Ownership is derived from each extension's parsed `CREATE TABLE`
/// statements, never declared — this is the boot-time guard against two
/// extensions silently diverging on a table both create.
fn validate_table_ownership(
    prepared: &[PreparedSchema],
    schema_extensions: &[std::sync::Arc<dyn Extension>],
) -> Result<(), LoaderError> {
    let mut owners: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for p in prepared {
        for table in &p.owned_tables {
            if let Some(prev) = owners.insert(table.as_str(), p.extension_id.as_str()) {
                if prev != p.extension_id {
                    return Err(LoaderError::DuplicateTableOwner {
                        table: table.clone(),
                        extension_a: prev.to_string(),
                        extension_b: p.extension_id.clone(),
                    });
                }
            }
        }
    }

    for ext in schema_extensions {
        let ext_id = ext.id();
        for table in ext.cross_extension_tables() {
            let owned_elsewhere = owners.get(table).is_some_and(|&owner| owner != ext_id);
            if !owned_elsewhere {
                return Err(LoaderError::CrossExtensionTableNotOwned {
                    extension: ext_id.to_string(),
                    table: table.to_string(),
                });
            }
        }
    }

    Ok(())
}

async fn execute_statements_transactional(
    db: &dyn DatabaseProvider,
    statements: &[String],
    extension_id: &str,
) -> Result<(), LoaderError> {
    if statements.is_empty() {
        return Ok(());
    }

    let mut tx =
        db.begin_transaction()
            .await
            .map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: extension_id.to_string(),
                message: format!("Failed to begin transaction: {e}"),
            })?;

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        if let Err(e) = tx.execute(&sql_str, &[]).await {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::SchemaInstallationFailed {
                extension: extension_id.to_string(),
                message: format!(
                    "Statement {n}/{total} failed: {e}{rollback_note}\nSQL:\n{statement}",
                    n = idx + 1,
                ),
            });
        }
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to commit transaction: {e}"),
        })?;

    Ok(())
}

async fn validate_extension_columns(
    db: &dyn DatabaseProvider,
    cols: &super::prepare::ColumnsToValidate,
    extension_id: &str,
) -> Result<(), LoaderError> {
    for column in &cols.columns {
        validate_single_column(db, &cols.schema, &cols.table, column, extension_id).await?;
    }
    Ok(())
}

async fn validate_single_column(
    db: &dyn DatabaseProvider,
    schema: &str,
    table: &str,
    column: &str,
    extension_id: &str,
) -> Result<(), LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT 1 FROM information_schema.columns WHERE table_schema = $1 AND table_name = \
              $2 AND column_name = $3",
            &[&schema, &table, &column],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to validate column '{column}': {e}"),
        })?;

    if result.rows.is_empty() {
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Required column '{column}' not found in table '{schema}.{table}'"),
        });
    }

    Ok(())
}

/// Holds the dedicated Postgres session that owns the bootstrap advisory lock.
///
/// `pg_advisory_lock` is session-scoped: only the backend that acquired the
/// lock can release it. The guard pins one [`PoolConnection`] for the install's
/// lifetime so acquire and release run on the same session. Non-Postgres
/// providers skip locking — bootstrap concurrency is a Postgres-only concern.
struct BootstrapLockGuard {
    conn: Option<PoolConnection<Postgres>>,
}

impl BootstrapLockGuard {
    async fn acquire(db: &dyn DatabaseProvider) -> Result<Self, LoaderError> {
        let Some(pool) = db.get_postgres_pool() else {
            return Ok(Self { conn: None });
        };

        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: "database".to_string(),
                message: format!("Failed to acquire bootstrap lock connection: {e}"),
            })?;

        sqlx::query!("SELECT pg_advisory_lock($1)", BOOTSTRAP_ADVISORY_LOCK_KEY)
            .execute(conn.as_mut())
            .await
            .map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: "database".to_string(),
                message: format!("Failed to acquire bootstrap advisory lock: {e}"),
            })?;

        debug!(
            key = BOOTSTRAP_ADVISORY_LOCK_KEY,
            "Acquired bootstrap advisory lock"
        );

        Ok(Self { conn: Some(conn) })
    }

    async fn release(mut self) {
        if let Some(mut conn) = self.conn.take() {
            if let Err(e) =
                sqlx::query_scalar!("SELECT pg_advisory_unlock($1)", BOOTSTRAP_ADVISORY_LOCK_KEY)
                    .fetch_one(conn.as_mut())
                    .await
            {
                warn!(
                    error = %e,
                    "Failed to release bootstrap advisory lock; connection recycle will clear it"
                );
            }
        }
    }
}

impl Drop for BootstrapLockGuard {
    fn drop(&mut self) {
        if self.conn.is_some() {
            warn!(
                key = BOOTSTRAP_ADVISORY_LOCK_KEY,
                "BootstrapLockGuard dropped without explicit release; lock will clear when the \
                 pooled connection recycles"
            );
        }
    }
}
