use anyhow::{bail, Result};
use std::path::PathBuf;
use systemprompt_core_database::{install_module_schemas, install_module_seeds, DatabaseProvider};
use systemprompt_models::modules::{Module, ModuleSchema, ModuleSeed};

use crate::AppContext;

pub async fn install_module(module: &Module) -> Result<()> {
    let app_context = AppContext::new().await?;
    install_module_with_db(module, app_context.db_pool().as_ref()).await
}

pub async fn install_module_with_db(module: &Module, db: &dyn DatabaseProvider) -> Result<()> {
    install_module_schemas(module, db, schema_path).await?;
    install_module_seeds(module, db, seed_path).await?;
    Ok(())
}

fn schema_path(module: &Module, schema: &ModuleSchema) -> Result<PathBuf> {
    let schema_path = module.path.join(&schema.file);

    if schema_path.exists() {
        return Ok(schema_path);
    }

    bail!(
        "Schema file not found for module '{}': {}",
        module.name,
        schema_path.display()
    )
}

fn seed_path(module: &Module, seed: &ModuleSeed) -> Result<PathBuf> {
    let seed_path = module.path.join(&seed.file);

    if seed_path.exists() {
        return Ok(seed_path);
    }

    bail!(
        "Seed file not found for module '{}': {}",
        module.name,
        seed_path.display()
    )
}
