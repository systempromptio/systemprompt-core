//! A core member that retires a table hands it, its rows and its ownership
//! record to the core member that declares it in the same install. An install
//! member never takes a table this way, and a core member cannot take a table
//! its recorded owner still declares.

use std::sync::Arc;

use systemprompt_database::{
    DbPool, MigrationConfig, SchemaInstallReport, SchemaMember, SchemaName, SchemaUnit,
    install_schema_members,
};
use systemprompt_extension::{ExtensionMetadata, LoaderError, SchemaDefinition, StandingTag};
use systemprompt_test_fixtures::DisposableDb;

const OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_table_owners.sql");
const OBJECT_OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_object_owners.sql");

fn member(
    id: &'static str,
    standing: StandingTag,
    schemas: Vec<SchemaDefinition>,
    retirements: Vec<SchemaDefinition>,
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
        retirements,
        cross_tables: vec![],
        purges: vec![],
        reassignments: vec![],
    })
}

fn ledger() -> Arc<dyn SchemaMember> {
    member(
        "database",
        StandingTag::Core,
        vec![
            SchemaDefinition::new("schema_table_owners", OWNERS_DDL),
            SchemaDefinition::new("schema_object_owners", OBJECT_OWNERS_DDL),
        ],
        vec![],
    )
}

fn links() -> SchemaDefinition {
    SchemaDefinition::new(
        "handed_links",
        "CREATE TABLE IF NOT EXISTS handed_links (id BIGINT PRIMARY KEY, body TEXT);
         CREATE INDEX IF NOT EXISTS idx_handed_links_body ON handed_links(body);",
    )
}

fn retired_links() -> SchemaDefinition {
    SchemaDefinition::sql_only("DROP TABLE IF EXISTS handed_links;")
}

async fn install(
    pool: &DbPool,
    members: Vec<Arc<dyn SchemaMember>>,
) -> Result<SchemaInstallReport, LoaderError> {
    install_schema_members(
        &members,
        pool.write(),
        &SchemaName::public(),
        MigrationConfig::default(),
    )
    .await
}

async fn scalar(pool: &DbPool, sql: &str) -> Option<String> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_optional(pool.write_pool().as_ref())
        .await
        .expect("probe")
}

async fn owners(pool: &DbPool) -> (Option<String>, Option<String>) {
    (
        scalar(
            pool,
            "SELECT extension_id FROM schema_table_owners WHERE table_name = 'public.handed_links'",
        )
        .await,
        scalar(
            pool,
            "SELECT extension_id FROM schema_object_owners \
             WHERE object_name = 'public.idx_handed_links_body'",
        )
        .await,
    )
}

async fn established(name: &str) -> (DisposableDb, DbPool) {
    let database = DisposableDb::empty(name).await;
    let pool = database.test_pool().await;
    install(
        &pool,
        vec![
            ledger(),
            member("giver", StandingTag::Core, vec![links()], vec![]),
            member("taker", StandingTag::Core, vec![], vec![]),
        ],
    )
    .await
    .expect("the giver installs its table");
    sqlx::query("INSERT INTO handed_links (id, body) VALUES (1, 'kept')")
        .execute(pool.write_pool().as_ref())
        .await
        .expect("seed row");
    (database, pool)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_retired_core_table_passes_to_the_core_member_declaring_it_in_one_install() {
    let (database, pool) = established("handover_core").await;
    assert_eq!(
        owners(&pool).await,
        (Some("giver".to_owned()), Some("giver".to_owned()))
    );

    let handed = || {
        vec![
            ledger(),
            member("giver", StandingTag::Core, vec![], vec![retired_links()]),
            member("taker", StandingTag::Core, vec![links()], vec![]),
        ]
    };
    install(&pool, handed())
        .await
        .expect("the hand-over installs");

    assert_eq!(
        owners(&pool).await,
        (Some("taker".to_owned()), Some("taker".to_owned())),
        "the table and its index pass in the one install"
    );
    assert_eq!(
        scalar(&pool, "SELECT body FROM handed_links WHERE id = 1")
            .await
            .as_deref(),
        Some("kept"),
        "the rows pass with the table"
    );
    install(&pool, handed())
        .await
        .expect("the next install is settled");
    assert_eq!(
        owners(&pool).await,
        (Some("taker".to_owned()), Some("taker".to_owned()))
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_core_member_cannot_take_a_table_its_owner_does_not_retire() {
    let (database, pool) = established("handover_unretired").await;

    let err = install(
        &pool,
        vec![
            ledger(),
            member("giver", StandingTag::Core, vec![], vec![]),
            member("taker", StandingTag::Core, vec![links()], vec![]),
        ],
    )
    .await
    .expect_err("a table its owner has not retired is not handed over");

    assert!(
        matches!(err, LoaderError::DuplicateTableOwner { .. }),
        "expected DuplicateTableOwner, got {err:?}"
    );
    assert_eq!(owners(&pool).await.0.as_deref(), Some("giver"));

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_member_never_takes_a_retired_core_table() {
    let (database, pool) = established("handover_install").await;

    let err = install(
        &pool,
        vec![
            ledger(),
            member("giver", StandingTag::Core, vec![], vec![retired_links()]),
            member("taker", StandingTag::Install, vec![links()], vec![]),
        ],
    )
    .await
    .expect_err("an install member is refused the core table");

    assert!(
        matches!(err, LoaderError::DuplicateTableOwner { .. }),
        "expected DuplicateTableOwner, got {err:?}"
    );
    assert_eq!(owners(&pool).await.0.as_deref(), Some("giver"));

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_absent_owner_hands_nothing_over() {
    let (database, pool) = established("handover_absent").await;

    let err = install(
        &pool,
        vec![
            ledger(),
            member("taker", StandingTag::Core, vec![links()], vec![]),
        ],
    )
    .await
    .expect_err("an owner outside the install keeps its table");

    assert!(
        matches!(err, LoaderError::DuplicateTableOwner { .. }),
        "expected DuplicateTableOwner, got {err:?}"
    );
    assert_eq!(owners(&pool).await.0.as_deref(), Some("giver"));

    drop(pool);
    database.drop_now().await;
}
