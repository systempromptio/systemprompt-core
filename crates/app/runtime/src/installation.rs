//! Module schema/seed installation helpers.
//!
//! These wrap the lower-level installers in `systemprompt-database` and
//! attach an [`AppContext`] when the caller does not already have one.

use crate::AppContext;
use crate::error::RuntimeResult;
use systemprompt_database::{
    DatabaseProvider, install_module_schemas_from_source, install_module_seeds_from_path,
};
use systemprompt_models::modules::Module;

pub async fn install_module(module: &Module) -> RuntimeResult<()> {
    let app_context = AppContext::new().await?;
    install_module_with_db(module, app_context.db_pool().as_ref()).await
}

pub async fn install_module_with_db(
    module: &Module,
    db: &dyn DatabaseProvider,
) -> RuntimeResult<()> {
    install_module_schemas_from_source(module, db).await?;
    install_module_seeds_from_path(module, db).await?;
    Ok(())
}
