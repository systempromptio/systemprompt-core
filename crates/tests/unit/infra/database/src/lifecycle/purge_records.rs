use std::sync::Arc;

use systemprompt_database::{
    DbPool, MigrationConfig, SchemaMember, SchemaUnit, install_schema_members,
};
use systemprompt_extension::{
    ExtensionMetadata, LoaderError, PurgeAdmissionError, PurgeDeclaration, SchemaDefinition,
    StandingTag,
};
use systemprompt_test_fixtures::DisposableDb;

const OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_table_owners.sql");
const RECORDS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_purge_records.sql");

fn unit(
    id: &'static str,
    standing: StandingTag,
    schemas: Vec<SchemaDefinition>,
    purges: Vec<PurgeDeclaration>,
) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id,
            name: id,
            version: "1.0.0",
        },
        standing,
        schemas,
        migrations: vec![],
        seeds: vec![],
        retirements: vec![],
        cross_tables: vec![],
        purges,
        reassignments: vec![],
    })
}

fn ledger() -> Arc<dyn SchemaMember> {
    unit(
        "database",
        StandingTag::Core,
        vec![
            SchemaDefinition::new("schema_table_owners", OWNERS_DDL),
            SchemaDefinition::new("schema_purge_records", RECORDS_DDL),
        ],
        vec![],
    )
}

fn table(name: &str) -> SchemaDefinition {
    SchemaDefinition::new(
        name,
        format!("CREATE TABLE IF NOT EXISTS {name} (id BIGINT PRIMARY KEY, user_id TEXT);"),
    )
}

fn purge(table: &str, column: &str) -> PurgeDeclaration {
    PurgeDeclaration::Table {
        table: table.to_owned(),
        column: column.to_owned(),
        predicate: None,
    }
}

fn notes(purges: Vec<PurgeDeclaration>) -> Arc<dyn SchemaMember> {
    unit(
        "notes",
        StandingTag::Install,
        vec![table("purge_notes")],
        purges,
    )
}

async fn install(pool: &DbPool, members: Vec<Arc<dyn SchemaMember>>) -> Result<(), LoaderError> {
    install_schema_members(&members, pool.write(), MigrationConfig::default())
        .await
        .map(drop)
}

async fn recorded(pool: &DbPool, member: &str) -> Option<Vec<PurgeDeclaration>> {
    sqlx::query_scalar::<_, sqlx::types::Json<Vec<PurgeDeclaration>>>(
        "SELECT declarations FROM schema_purge_records WHERE extension_id = $1",
    )
    .bind(member)
    .fetch_optional(pool.write_pool().as_ref())
    .await
    .expect("record lookup")
    .map(|json| json.0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_records_purge_declarations_that_outlive_the_member_being_disabled() {
    let database = DisposableDb::empty("purge_records_kept").await;
    let pool = database.test_pool().await;
    let declared = vec![purge("purge_notes", "user_id")];
    install(&pool, vec![ledger(), notes(declared.clone())])
        .await
        .expect("the member installs");
    assert_eq!(recorded(&pool, "notes").await, Some(declared.clone()));
    assert_eq!(
        recorded(&pool, "database").await,
        Some(vec![]),
        "a member with no declarations is recorded as vouched-for"
    );

    install(&pool, vec![ledger()])
        .await
        .expect("installs with notes disabled");
    assert_eq!(
        recorded(&pool, "notes").await,
        Some(declared),
        "a disabled member's record is kept"
    );

    install(&pool, vec![ledger(), notes(vec![])])
        .await
        .expect("the member re-enables with no declarations");
    assert_eq!(
        recorded(&pool, "notes").await,
        Some(vec![]),
        "the record is replaced whole"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_purging_another_members_table_is_refused_before_any_record() {
    let database = DisposableDb::empty("purge_records_foreign").await;
    let pool = database.test_pool().await;
    let claimant = unit(
        "claimant",
        StandingTag::Install,
        vec![table("claimant_rows")],
        vec![purge("purge_notes", "user_id")],
    );
    let err = install(&pool, vec![ledger(), notes(vec![]), claimant])
        .await
        .expect_err("a purge over a peer's table is refused");
    match err {
        LoaderError::PurgeAdmission(PurgeAdmissionError::ForeignTable { member, table }) => {
            assert_eq!(member.as_str(), "claimant");
            assert_eq!(table, "purge_notes");
        },
        other => panic!("unexpected error: {other:?}"),
    }

    let own = unit(
        "claimant",
        StandingTag::Install,
        vec![table("claimant_rows")],
        vec![purge("claimant_rows", "user_id")],
    );
    install(&pool, vec![ledger(), notes(vec![]), own])
        .await
        .expect("a purge over its own table is admitted");
    assert!(recorded(&pool, "claimant").await.is_some());

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_purge_column_that_is_not_an_identifier_is_refused() {
    let database = DisposableDb::empty("purge_records_ident").await;
    let pool = database.test_pool().await;
    let err = install(
        &pool,
        vec![
            ledger(),
            notes(vec![purge("purge_notes", "user_id = user_id OR true")]),
        ],
    )
    .await
    .expect_err("a non-identifier column is refused");
    assert!(matches!(
        err,
        LoaderError::SchemaInstallationStepFailed { .. }
    ));
    install(
        &pool,
        vec![ledger(), notes(vec![purge("purge_notes", "user_id")])],
    )
    .await
    .expect("an identifier column is admitted");

    drop(pool);
    database.drop_now().await;
}
