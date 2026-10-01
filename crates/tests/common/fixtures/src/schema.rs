//! Schema-installation shorthands for lifecycle tests.
//!
//! Production boots through `install_extension_schemas_full`; tests that do
//! not care about migration tuning or disabled extensions call these instead.

use systemprompt_database::{
    install_extension_schemas_full, DatabaseProvider, MigrationConfig, SchemaInstallReport,
};
use systemprompt_extension::{ExtensionRegistry, LoaderError};

pub async fn install_extension_schemas(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
) -> Result<SchemaInstallReport, LoaderError> {
    install_extension_schemas_with_config(registry, db, &[]).await
}

pub async fn install_extension_schemas_with_config(
    registry: &ExtensionRegistry,
    db: &dyn DatabaseProvider,
    disabled_extensions: &[String],
) -> Result<SchemaInstallReport, LoaderError> {
    install_extension_schemas_full(registry, db, disabled_extensions, MigrationConfig::default())
        .await
}
