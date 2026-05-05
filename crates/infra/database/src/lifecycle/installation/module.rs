//! Schema and seed installation from on-disk
//! [`systemprompt_models::modules::Module`] descriptors (legacy loader path).

use std::path::Path;

use systemprompt_extension::{SchemaSource, SeedSource};
use systemprompt_models::modules::{Module, ModuleSchema};

use super::util::table_exists;
use crate::error::{DatabaseResult, RepositoryError};
use crate::services::{DatabaseProvider, SqlExecutor};

#[derive(Debug, Clone, Copy)]
pub struct ModuleInstaller;

impl ModuleInstaller {
    pub async fn install(module: &Module, db: &dyn DatabaseProvider) -> DatabaseResult<()> {
        install_module_schemas_from_source(module, db).await?;
        install_module_seeds_from_path(module, db).await?;
        Ok(())
    }
}

pub async fn install_module_schemas_from_source(
    module: &Module,
    db: &dyn DatabaseProvider,
) -> DatabaseResult<()> {
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

fn read_module_schema_sql(module: &Module, schema: &ModuleSchema) -> DatabaseResult<String> {
    match &schema.sql {
        SchemaSource::Inline(sql) => Ok(sql.clone()),
        SchemaSource::File(relative_path) => {
            let full_path = module.path.join(relative_path);
            std::fs::read_to_string(&full_path).map_err(|e| {
                RepositoryError::Internal(format!(
                    "Failed to read schema file '{}' for module '{}': {e}",
                    full_path.display(),
                    module.name
                ))
            })
        },
    }
}

pub async fn install_module_seeds_from_path(
    module: &Module,
    db: &dyn DatabaseProvider,
) -> DatabaseResult<()> {
    let Some(seeds) = &module.seeds else {
        return Ok(());
    };

    for seed in seeds {
        let sql = match &seed.sql {
            SeedSource::Inline(sql) => sql.clone(),
            SeedSource::File(relative_path) => {
                let seed_path = module.path.join(relative_path);
                if !seed_path.exists() {
                    return Err(RepositoryError::Internal(format!(
                        "Seed file not found for module '{}': {}",
                        module.name,
                        seed_path.display()
                    )));
                }
                std::fs::read_to_string(&seed_path).map_err(|e| {
                    RepositoryError::Internal(format!(
                        "Failed to read seed file '{}' for module '{}': {e}",
                        seed_path.display(),
                        module.name
                    ))
                })?
            },
        };
        SqlExecutor::execute_statements_parsed(db, &sql).await?;
    }

    Ok(())
}

pub async fn install_schema(db: &dyn DatabaseProvider, schema_path: &Path) -> DatabaseResult<()> {
    let schema_content = std::fs::read_to_string(schema_path).map_err(|e| {
        RepositoryError::Internal(format!(
            "Failed to read schema file '{}': {e}",
            schema_path.display()
        ))
    })?;
    SqlExecutor::execute_statements_parsed(db, &schema_content).await
}

pub async fn install_seed(db: &dyn DatabaseProvider, seed_path: &Path) -> DatabaseResult<()> {
    let seed_content = std::fs::read_to_string(seed_path).map_err(|e| {
        RepositoryError::Internal(format!(
            "Failed to read seed file '{}': {e}",
            seed_path.display()
        ))
    })?;
    SqlExecutor::execute_statements_parsed(db, &seed_content).await
}
