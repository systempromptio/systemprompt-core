use std::sync::Arc;

use systemprompt_database::{
    DatabaseProvider, MigrationConfig, SchemaInstallReport, SchemaMember, install_admitted_schemas,
    schema_members,
};
use systemprompt_extension::{Extension, LoaderError};
use systemprompt_test_fixtures::fixture_admitted_of;

pub fn member(ext: impl Extension) -> Arc<dyn SchemaMember> {
    schema_members(fixture_admitted_of(vec![Box::new(ext)]).built())
        .pop()
        .expect("the member declares a schema")
}

pub async fn install(
    members: Vec<Box<dyn Extension>>,
    db: &dyn DatabaseProvider,
) -> Result<SchemaInstallReport, LoaderError> {
    install_admitted_schemas(
        fixture_admitted_of(members).built(),
        db,
        MigrationConfig::default(),
    )
    .await
}
