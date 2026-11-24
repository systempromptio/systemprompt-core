use std::sync::Arc;

use systemprompt_database::{
    DatabaseProvider, MigrationConfig, SchemaInstallReport, SchemaName, SchemaUnit,
    install_admitted_schemas,
};
use systemprompt_extension::{Admitted, Extension, LoaderError, StandingTag, purge_declarations};
use systemprompt_test_fixtures::{admitted_member, fixture_admitted_of};

pub(super) fn admitted_of(members: Vec<Box<dyn Extension>>) -> Arc<Admitted> {
    fixture_admitted_of(members)
}

pub(super) async fn install(
    admitted: &Admitted,
    db: &dyn DatabaseProvider,
    target: &SchemaName,
) -> Result<SchemaInstallReport, LoaderError> {
    install_admitted_schemas(admitted.built(), db, target, MigrationConfig::default()).await
}

pub(super) fn unit(ext: &dyn Extension) -> SchemaUnit {
    let member = admitted_member(ext, StandingTag::Core).expect("the stub contributes");
    SchemaUnit {
        metadata: member.metadata,
        standing: member.standing,
        schemas: member.sink.schemas().to_vec(),
        migrations: member.sink.migrations().to_vec(),
        seeds: member.sink.seeds().to_vec(),
        retirements: member.sink.retirements().to_vec(),
        cross_tables: member.sink.cross_tables().to_vec(),
        purges: purge_declarations(&member.sink),
        reassignments: member.sink.owner_reassignments().to_vec(),
    }
}
