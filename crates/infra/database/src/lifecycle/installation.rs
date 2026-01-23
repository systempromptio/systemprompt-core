use super::migrations::MigrationService;
use crate::services::{DatabaseProvider, SqlExecutor};
use anyhow::Result;
use std::path::Path;
use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError, SchemaSource, SeedSource};
use systemprompt_models::modules::{Module, ModuleSchema};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy)]
pub struct ModuleInstaller;

impl ModuleInstaller {
    pub async fn install(module: &Module, db: &dyn DatabaseProvider) -> Result<()> {
        install_module_schemas_from_source(module, db).await?;
        install_module_seeds_from_path(module, db).await?;
        Ok(())
    }
}

pub async fn install_module_schemas_from_source(
    module: &Module,
    db: &dyn DatabaseProvider,
) -> Result<()> {
    let Some(schemas) = &module.schemas else {
        return Ok(());
    };

    for schema in schemas {
        if schema.table.is_empty() {
            let sql = read_module_schema_sql(module, schema)?;
            SqlExecutor::execute_statements_parsed(db, &sql).await?;
            continue;
        }

        if !table_exists(db, &schema.table).await? {
            let sql = read_module_schema_sql(module, schema)?;
            SqlExecutor::execute_statements_parsed(db, &sql).await?;
        }
    }

    Ok(())
}

fn read_module_schema_sql(module: &Module, schema: &ModuleSchema) -> Result<String> {
    match &schema.sql {
        SchemaSource::Inline(sql) => Ok(sql.clone()),
        SchemaSource::File(relative_path) => {
            let full_path = module.path.join(relative_path);
            std::fs::read_to_string(&full_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read schema file '{}' for module '{}': {e}",
                    full_path.display(),
                    module.name
                )
            })
        },
    }
}

pub async fn install_module_seeds_from_path(
    module: &Module,
    db: &dyn DatabaseProvider,
) -> Result<()> {
    let Some(seeds) = &module.seeds else {
        return Ok(());
    };

    for seed in seeds {
        let sql = match &seed.sql {
            SeedSource::Inline(sql) => sql.clone(),
            SeedSource::File(relative_path) => {
                let seed_path = module.path.join(relative_path);
                if !seed_path.exists() {
                    anyhow::bail!(
                        "Seed file not found for module '{}': {}",
                        module.name,
                        seed_path.display()
                    );
                }
                std::fs::read_to_string(&seed_path)?
            },
        };
        SqlExecutor::execute_statements_parsed(db, &sql).await?;
    }

    Ok(())
}

pub async fn install_schema(db: &dyn DatabaseProvider, schema_path: &Path) -> Result<()> {
    let schema_content = std::fs::read_to_string(schema_path)?;
    SqlExecutor::execute_statements_parsed(db, &schema_content).await
}

pub async fn install_seed(db: &dyn DatabaseProvider, seed_path: &Path) -> Result<()> {
    let seed_content = std::fs::read_to_string(seed_path)?;
    SqlExecutor::execute_statements_parsed(db, &seed_content).await
}

async fn table_exists(db: &dyn DatabaseProvider, table_name: &str) -> Result<bool> {
    let result = db
        .query_raw_with(
            &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1) as exists",
            vec![serde_json::Value::String(table_name.to_string())],
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, table = %table_name, "Database error checking table existence");
            anyhow::anyhow!("Database error checking table '{}': {}", table_name, e)
        })?;

    let exists = result
        .rows
        .first()
        .and_then(|row| row.get("exists"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(exists)
}

pub async fn install_extension_schemas(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
) -> std::result::Result<(), LoaderError> {
    install_extension_schemas_with_config(registry, db, &[]).await
}

pub async fn install_extension_schemas_with_config(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
) -> std::result::Result<(), LoaderError> {
    let schema_extensions = registry.enabled_schema_extensions(disabled_extensions);

    if schema_extensions.is_empty() {
        log_no_schemas();
        return Ok(());
    }

    log_installing_schemas(schema_extensions.len());

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

    log_installation_complete();
    Ok(())
}

fn log_no_schemas() {
    info!("No extension schemas to install");
}

fn log_installing_schemas(count: usize) {
    info!("Installing schemas for {} extensions", count);
}

fn log_installation_complete() {
    info!("Extension schema installation complete");
}

async fn install_extension_schema(
    ext: &dyn Extension,
    db: &dyn DatabaseProvider,
) -> std::result::Result<(), LoaderError> {
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

    for schema in &schemas {
        install_single_schema(db, schema, &extension_id).await?;
    }

    Ok(())
}

async fn install_single_schema(
    db: &dyn DatabaseProvider,
    schema: &systemprompt_extension::SchemaDefinition,
    extension_id: &str,
) -> std::result::Result<(), LoaderError> {
    if check_table_exists_for_extension(db, &schema.table, extension_id).await? {
        debug!("  Table '{}' already exists, skipping", schema.table);
        return Ok(());
    }

    let sql = read_schema_sql(schema, extension_id)?;
    execute_schema_sql(db, &sql, &schema.table, extension_id).await?;

    if !schema.required_columns.is_empty() {
        validate_extension_columns(db, &schema.table, &schema.required_columns, extension_id)
            .await?;
    }

    Ok(())
}

async fn check_table_exists_for_extension(
    db: &dyn DatabaseProvider,
    table: &str,
    extension_id: &str,
) -> std::result::Result<bool, LoaderError> {
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
) -> std::result::Result<String, LoaderError> {
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

async fn execute_schema_sql(
    db: &dyn DatabaseProvider,
    sql: &str,
    table: &str,
    extension_id: &str,
) -> std::result::Result<(), LoaderError> {
    debug!("  Creating table '{}'", table);
    SqlExecutor::execute_statements_parsed(db, sql)
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to create table '{}': {e}", table),
        })
}

async fn validate_extension_columns(
    db: &dyn DatabaseProvider,
    table: &str,
    required_columns: &[String],
    extension_id: &str,
) -> std::result::Result<(), LoaderError> {
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
) -> std::result::Result<(), LoaderError> {
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
