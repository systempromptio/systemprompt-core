//! Schema and seed installation from on-disk
//! [`systemprompt_models::modules::Module`] descriptors (legacy loader path).

use std::path::Path;

use anyhow::Result;
use systemprompt_extension::{SchemaSource, SeedSource};
use systemprompt_models::modules::{Module, ModuleSchema};

use super::util::table_exists;
use crate::services::{DatabaseProvider, SqlExecutor};

/// Aggregator that runs every per-module installation step in the right order.
#[derive(Debug, Clone, Copy)]
pub struct ModuleInstaller;

impl ModuleInstaller {
    /// Install schemas (DDL) followed by seeds (DML) for `module`.
    pub async fn install(module: &Module, db: &dyn DatabaseProvider) -> Result<()> {
        install_module_schemas_from_source(module, db).await?;
        install_module_seeds_from_path(module, db).await?;
        Ok(())
    }
}

/// Install every schema declared by `module`. Schemas with an empty `table`
/// name are always run; schemas with a `table` are skipped if that table
/// already exists.
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

/// Install every seed declared by `module`. Seeds always run unconditionally.
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

/// Read a SQL file and run it as a parsed statement batch.
pub async fn install_schema(db: &dyn DatabaseProvider, schema_path: &Path) -> Result<()> {
    let schema_content = std::fs::read_to_string(schema_path)?;
    SqlExecutor::execute_statements_parsed(db, &schema_content).await
}

/// Read a seed SQL file and run it as a parsed statement batch.
pub async fn install_seed(db: &dyn DatabaseProvider, seed_path: &Path) -> Result<()> {
    let seed_content = std::fs::read_to_string(seed_path)?;
    SqlExecutor::execute_statements_parsed(db, &seed_content).await
}
