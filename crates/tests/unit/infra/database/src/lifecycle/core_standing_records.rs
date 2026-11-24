use std::sync::Arc;

use systemprompt_database::{
    DbPool, MigrationConfig, SchemaMember, SchemaUnit, install_schema_members,
};
use systemprompt_extension::{
    ExtensionMetadata, LoaderError, PurgeDeclaration, SchemaDefinition, StandingTag,
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

fn facts() -> SchemaDefinition {
    SchemaDefinition::new(
        "standing_facts",
        "CREATE TABLE IF NOT EXISTS standing_facts (id BIGINT PRIMARY KEY, user_id TEXT);",
    )
}

fn other() -> SchemaDefinition {
    SchemaDefinition::new(
        "standing_other",
        "CREATE TABLE IF NOT EXISTS standing_other (id BIGINT PRIMARY KEY, user_id TEXT);",
    )
}

fn purge(table: &str) -> PurgeDeclaration {
    PurgeDeclaration::Table {
        table: table.to_owned(),
        column: "user_id".to_owned(),
        predicate: None,
    }
}

async fn install(pool: &DbPool, members: Vec<Arc<dyn SchemaMember>>) -> Result<(), LoaderError> {
    install_schema_members(&members, pool.write(), MigrationConfig::default())
        .await
        .map(drop)
}

async fn recorded_standing(pool: &DbPool, sql: &'static str, key: &str) -> Option<String> {
    sqlx::query_scalar(sql)
        .bind(key)
        .fetch_optional(pool.write_pool().as_ref())
        .await
        .expect("standing lookup")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_with_a_core_members_id_cannot_adopt_its_table() {
    let database = DisposableDb::empty("standing_table").await;
    let pool = database.test_pool().await;
    install(
        &pool,
        vec![
            ledger(),
            unit("squatted", StandingTag::Core, vec![facts()], vec![]),
        ],
    )
    .await
    .expect("the core member installs");
    assert_eq!(
        recorded_standing(
            &pool,
            "SELECT standing FROM schema_table_owners WHERE table_name = $1",
            "public.standing_facts",
        )
        .await
        .as_deref(),
        Some("core")
    );

    let err = install(
        &pool,
        vec![
            ledger(),
            unit("squatted", StandingTag::Install, vec![facts()], vec![]),
        ],
    )
    .await
    .expect_err("an install claiming a core-recorded table is refused");
    assert!(
        matches!(
            &err,
            LoaderError::CoreRecordClaimedByInstall { kind: "table", owner, .. }
                if owner.as_str() == "squatted"
        ),
        "unexpected error: {err:?}"
    );

    install(
        &pool,
        vec![
            ledger(),
            unit("squatted", StandingTag::Core, vec![facts()], vec![]),
        ],
    )
    .await
    .expect("the core owner still installs over its own record");

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_with_a_core_members_id_cannot_replace_its_purge_records() {
    let database = DisposableDb::empty("standing_purges").await;
    let pool = database.test_pool().await;
    install(
        &pool,
        vec![
            ledger(),
            unit(
                "squatted",
                StandingTag::Core,
                vec![facts()],
                vec![purge("standing_facts")],
            ),
        ],
    )
    .await
    .expect("the core member installs");

    let err = install(
        &pool,
        vec![
            ledger(),
            unit(
                "squatted",
                StandingTag::Install,
                vec![other()],
                vec![purge("standing_other")],
            ),
        ],
    )
    .await
    .expect_err("an install replacing core purge records is refused");
    assert!(
        matches!(
            &err,
            LoaderError::CoreRecordClaimedByInstall {
                kind: "user purge records",
                ..
            }
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        recorded_standing(
            &pool,
            "SELECT standing FROM schema_purge_records WHERE extension_id = $1",
            "squatted",
        )
        .await
        .as_deref(),
        Some("core")
    );

    install(
        &pool,
        vec![
            ledger(),
            unit(
                "fresh_install",
                StandingTag::Install,
                vec![other()],
                vec![purge("standing_other")],
            ),
        ],
    )
    .await
    .expect("an install with its own id records its purges");
    assert_eq!(
        recorded_standing(
            &pool,
            "SELECT standing FROM schema_purge_records WHERE extension_id = $1",
            "fresh_install",
        )
        .await
        .as_deref(),
        Some("install")
    );

    drop(pool);
    database.drop_now().await;
}
