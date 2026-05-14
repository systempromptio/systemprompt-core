//! Schema installation from compile-time-registered
//! [`systemprompt_extension::Extension`] instances (the modern path).
//!
//! Invariants:
//! - Every `SchemaDefinition.sql` is executed on every boot. Schemas are
//!   expected to be idempotent (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF
//!   NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`). The previous "skip if table
//!   already exists" optimisation silently dropped post-install ALTERs and
//!   defeated the safety-net pattern; it is intentionally gone.
//! - The full set of statements for one extension runs inside a single
//!   transaction. On failure, the transaction is rolled back and the failing
//!   statement (with its 1-based index and SQL text) is surfaced in the error.
//! - A session-scoped advisory lock serialises concurrent boot processes so
//!   rolling deploys or accidental double-invocations cannot interleave DDL.

use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError};
use tracing::{debug, info};

use crate::lifecycle::migrations::{MigrationConfig, MigrationService};
use crate::services::schema_additivity::{compute_additive_alters, parse_declared_tables};
use crate::services::{DatabaseProvider, SqlExecutor};

/// Stable 64-bit key for `pg_advisory_lock`. Chosen as a constant so all
/// `systemprompt`-managed processes serialise on the same lock.
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

    acquire_advisory_lock(db).await?;

    let result = run_install(db, &schema_extensions, migration_config).await;

    if let Err(e) = release_advisory_lock(db).await {
        tracing::warn!(error = %e, "Failed to release bootstrap advisory lock");
    }

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

    for ext in schema_extensions {
        install_extension_schema(ext.as_ref(), db).await?;

        if ext.has_migrations() {
            debug!(
                extension = %ext.id(),
                "Running pending migrations"
            );
            migration_service
                .run_pending_migrations(ext.as_ref())
                .await?;
        }
    }
    Ok(())
}

async fn install_extension_schema(
    ext: &dyn Extension,
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    let schemas = ext.schemas();
    let extension_id = ext.metadata().id.to_string();

    if schemas.is_empty() {
        return Ok(());
    }

    debug!(
        "Installing {} schema(s) for extension '{}' (weight: {})",
        schemas.len(),
        extension_id,
        ext.migration_weight()
    );

    let mut all_sql = Vec::new();
    let mut schemas_to_validate = Vec::new();

    for schema in &schemas {
        all_sql.push(schema.sql.as_str());

        if !schema.required_columns.is_empty() {
            schemas_to_validate.push(schema);
        }
    }

    let combined = all_sql.join("\n");
    let parsed = SqlExecutor::parse_sql_statements(&combined).map_err(|e| {
        LoaderError::SchemaInstallationFailed {
            extension: extension_id.clone(),
            message: format!("SQL parse failed: {e}"),
        }
    })?;

    let declared_tables = parse_declared_tables(&combined);
    let additive_alters = compute_additive_alters(db, &declared_tables)
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.clone(),
            message: format!("Schema-additivity check failed: {e}"),
        })?;
    for alter in &additive_alters {
        debug!(
            extension = %extension_id,
            sql = %alter,
            "Auto-emitting additive ALTER for legacy table"
        );
    }

    let mut statements = additive_alters;
    statements.extend(parsed);

    execute_statements_transactional(db, &statements, &extension_id).await?;

    for schema in schemas_to_validate {
        validate_extension_columns(db, &schema.table, &schema.required_columns, &extension_id)
            .await?;
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
    table: &str,
    required_columns: &[String],
    extension_id: &str,
) -> Result<(), LoaderError> {
    for column in required_columns {
        validate_single_column(db, table, column, extension_id).await?;
    }
    Ok(())
}

async fn validate_single_column(
    db: &dyn DatabaseProvider,
    table: &str,
    column: &str,
    extension_id: &str,
) -> Result<(), LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT 1 FROM information_schema.columns WHERE table_schema = 'public' AND \
              table_name = $1 AND column_name = $2",
            vec![
                serde_json::Value::String(table.to_string()),
                serde_json::Value::String(column.to_string()),
            ],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to validate column '{column}': {e}"),
        })?;

    if result.rows.is_empty() {
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Required column '{column}' not found in table '{table}'"),
        });
    }

    Ok(())
}

async fn acquire_advisory_lock(db: &dyn DatabaseProvider) -> Result<(), LoaderError> {
    let sql = format!("SELECT pg_advisory_lock({BOOTSTRAP_ADVISORY_LOCK_KEY})");
    db.execute_raw(&sql)
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: "database".to_string(),
            message: format!("Failed to acquire bootstrap advisory lock: {e}"),
        })?;
    debug!(
        key = BOOTSTRAP_ADVISORY_LOCK_KEY,
        "Acquired bootstrap advisory lock"
    );
    Ok(())
}

async fn release_advisory_lock(
    db: &dyn DatabaseProvider,
) -> Result<(), crate::error::RepositoryError> {
    let sql = format!("SELECT pg_advisory_unlock({BOOTSTRAP_ADVISORY_LOCK_KEY})");
    db.execute_raw(&sql).await
}
