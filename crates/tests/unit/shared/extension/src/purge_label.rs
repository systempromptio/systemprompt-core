use systemprompt_extension::{
    AdmittedPurge, PurgeAdmissionError, PurgeClaim, PurgeDeclaration, SchemaDefinition,
    UserPurgeTable,
};
use systemprompt_identifiers::ExtensionId;

fn purge(table: &'static str) -> PurgeDeclaration {
    PurgeDeclaration::from(UserPurgeTable {
        table,
        column: "created_by",
        predicate: None,
    })
}

fn admit_install(
    schema: SchemaDefinition,
    declaration: PurgeDeclaration,
) -> Result<AdmittedPurge, PurgeAdmissionError> {
    let core = ExtensionId::new("oauth");
    let install = ExtensionId::new("acme");
    let core_schemas = [SchemaDefinition::new(
        "oauth_clients",
        "CREATE TABLE oauth_clients (created_by TEXT)",
    )];
    let install_schemas = [schema];
    let declarations = [declaration];
    AdmittedPurge::admit(&[
        PurgeClaim {
            owner: &core,
            schemas: &core_schemas,
            declarations: &[],
        },
        PurgeClaim {
            owner: &install,
            schemas: &install_schemas,
            declarations: &declarations,
        },
    ])
}

#[test]
fn a_schema_labelled_with_a_core_table_does_not_own_it() {
    let refused = admit_install(
        SchemaDefinition::new(
            "oauth_clients",
            "CREATE TABLE IF NOT EXISTS acme_x (created_by TEXT)",
        ),
        purge("oauth_clients"),
    );
    assert_eq!(
        refused.unwrap_err(),
        PurgeAdmissionError::ForeignTable {
            member: ExtensionId::new("acme"),
            table: "oauth_clients".to_owned(),
        }
    );
}

#[test]
fn a_label_matching_the_created_table_is_admitted() {
    let admitted = admit_install(
        SchemaDefinition::new(
            "acme_x",
            "CREATE TABLE IF NOT EXISTS acme_x (created_by TEXT)",
        ),
        purge("acme_x"),
    )
    .expect("a member purging the table its SQL creates is admitted");
    assert_eq!(admitted.entries().len(), 1);
}

#[test]
fn an_unlabelled_schema_owns_the_tables_its_sql_creates() {
    admit_install(
        SchemaDefinition::new("acme", "CREATE TABLE public.acme_y (created_by TEXT)"),
        purge("acme_y"),
    )
    .expect("a public-qualified created table is the member's own");
}

#[test]
fn schema_sql_that_does_not_parse_refuses_admission() {
    let refused = admit_install(
        SchemaDefinition::new("acme_x", "CREATE TABLE ((("),
        purge("acme_x"),
    );
    assert!(matches!(
        refused,
        Err(PurgeAdmissionError::UnparseableSchema { .. })
    ));
}
