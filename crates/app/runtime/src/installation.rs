use anyhow::Result;
use systemprompt_database::{
    install_module_schemas_from_source, install_module_seeds_from_path, DatabaseProvider,
};
use systemprompt_models::modules::Module;

use crate::AppContext;

pub async fn install_module(module: &Module) -> Result<()> {
    let app_context = AppContext::new().await?;
    install_module_with_db(module, app_context.db_pool().as_ref()).await
}

pub async fn install_module_with_db(module: &Module, db: &dyn DatabaseProvider) -> Result<()> {
    install_module_schemas_from_source(module, db).await?;
    install_module_seeds_from_path(module, db).await?;
    Ok(())
}
