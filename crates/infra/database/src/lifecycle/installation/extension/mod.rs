//! Schema installation for compile-time-registered
//! [`systemprompt_extension::Extension`] instances.
//!
//! Installation runs globally in three phases — structural DDL, then
//! migrations, then dependent DDL — so a legacy database reaches its target
//! shape before any `CREATE INDEX`/`VIEW` references a migration-added column.
//! A session-scoped advisory lock serialises concurrent boots. See
//! `instructions/information/migrations.md`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod lock;
mod validation;

use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError};
use tracing::{debug, info};

use self::lock::BootstrapLockGuard;
use self::validation::{validate_extension_columns, validate_table_ownership};
use super::prepare::{PreparedSchema, prepare_extension_schema};
use super::seeds::apply_seeds;
use crate::lifecycle::migrations::{MigrationConfig, MigrationService};
use crate::services::DatabaseProvider;

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
                extension: extension_id.to_owned(),
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
                extension: extension_id.to_owned(),
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
            extension: extension_id.to_owned(),
            message: format!("Failed to commit transaction: {e}"),
        })?;

    Ok(())
}
