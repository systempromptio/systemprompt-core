//! Schema installation from compile-time-registered
//! [`systemprompt_extension::Extension`] instances (the modern path).

use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError, SchemaSource};
use tracing::{debug, info, warn};

use super::util::table_exists;
use crate::lifecycle::migrations::MigrationService;
use crate::services::{DatabaseProvider, SqlExecutor};

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
    let schema_extensions = registry.enabled_schema_extensions(disabled_extensions);

    if schema_extensions.is_empty() {
        info!("No extension schemas to install");
        return Ok(());
    }

    info!(
        "Installing schemas for {} extensions",
        schema_extensions.len()
    );

    let migration_service = MigrationService::new(db);

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

    info!("Extension schema installation complete");
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
        if !schema.table.is_empty()
            && check_table_exists_for_extension(db, &schema.table, &extension_id).await?
        {
            debug!("  Table '{}' already exists, skipping", schema.table);
            continue;
        }

        let sql = read_schema_sql(schema, &extension_id)?;
        all_sql.push(sql);

        if !schema.required_columns.is_empty() {
            schemas_to_validate.push(schema);
        }
    }

    if all_sql.is_empty() {
        return Ok(());
    }

    let combined = all_sql.join("\n");
    let statements = SqlExecutor::parse_sql_statements(&combined);

    if !statements.is_empty() {
        let batch = statements.join("\n");
        if let Err(batch_err) = db.execute_raw(&batch).await {
            debug!(
                extension = %extension_id,
                error = %batch_err,
                "Batch execution failed, falling back to per-statement execution"
            );
            for statement in &statements {
                db.execute_raw(statement).await.map_err(|e| {
                    LoaderError::SchemaInstallationFailed {
                        extension: extension_id.clone(),
                        message: format!("Failed to execute SQL statement: {e}\n{statement}"),
                    }
                })?;
            }
        }
    }

    for schema in schemas_to_validate {
        validate_extension_columns(db, &schema.table, &schema.required_columns, &extension_id)
            .await?;
    }

    Ok(())
}

async fn check_table_exists_for_extension(
    db: &dyn DatabaseProvider,
    table: &str,
    extension_id: &str,
) -> Result<bool, LoaderError> {
    table_exists(db, table)
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to check table existence: {e}"),
        })
}

fn read_schema_sql(
    schema: &systemprompt_extension::SchemaDefinition,
    extension_id: &str,
) -> Result<String, LoaderError> {
    match &schema.sql {
        SchemaSource::Inline(sql) => Ok(sql.clone()),
        SchemaSource::File(path) => {
            std::fs::read_to_string(path).map_err(|e| LoaderError::SchemaInstallationFailed {
                extension: extension_id.to_string(),
                message: format!("Failed to read schema file '{}': {e}", path.display()),
            })
        },
    }
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
        warn!(
            "Extension '{}': Required column '{}' not found in table '{}'",
            extension_id, column, table
        );
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Required column '{column}' not found in table '{table}'"),
        });
    }

    Ok(())
}
