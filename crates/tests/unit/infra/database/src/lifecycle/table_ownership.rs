use std::sync::Arc;

use systemprompt_database::{
    DbPool, MigrationConfig, SchemaInstallReport, SchemaMember, SchemaUnit, install_schema_members,
};
use systemprompt_extension::{
    ExtensionMetadata, LoaderError, Migration, SchemaDefinition, StandingTag,
};
use systemprompt_test_fixtures::DisposableDb;

const OWNERS_DDL: &str =
    include_str!("../../../../../../infra/database/schema/schema_table_owners.sql");

fn unit(
    id: &'static str,
    schemas: Vec<SchemaDefinition>,
    migrations: Vec<Migration>,
) -> Arc<dyn SchemaMember> {
    Arc::new(SchemaUnit {
        metadata: ExtensionMetadata {
            id,
            name: id,
            version: "1.0.0",
        },
        standing: StandingTag::Install,
        schemas,
        migrations,
        seeds: vec![],
        retirements: vec![],
        cross_tables: vec![],
        purges: vec![],
    })
}

fn ledger() -> Arc<dyn SchemaMember> {
    unit(
        "database",
        vec![SchemaDefinition::new("schema_table_owners", OWNERS_DDL)],
        vec![],
    )
}

fn table(name: &str) -> SchemaDefinition {
    SchemaDefinition::new(
        name,
        format!("CREATE TABLE IF NOT EXISTS {name} (id BIGINT PRIMARY KEY, body TEXT);"),
    )
}

fn notes_owner() -> Arc<dyn SchemaMember> {
    unit("notes_owner", vec![table("owned_notes")], vec![])
}

async fn install(
    pool: &DbPool,
    members: Vec<Arc<dyn SchemaMember>>,
) -> Result<SchemaInstallReport, LoaderError> {
    install_schema_members(&members, pool.write(), MigrationConfig::default()).await
}

async fn recorded_owner(pool: &DbPool, table: &str) -> Option<String> {
    sqlx::query_scalar("SELECT extension_id FROM schema_table_owners WHERE table_name = $1")
        .bind(format!("public.{table}"))
        .fetch_optional(pool.write_pool().as_ref())
        .await
        .expect("owner lookup")
}

async fn note_count(pool: &DbPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM owned_notes")
        .fetch_one(pool.write_pool().as_ref())
        .await
        .expect("count")
}

async fn seed_note(pool: &DbPool) {
    sqlx::query("INSERT INTO owned_notes (id, body) VALUES (1, 'private')")
        .execute(pool.write_pool().as_ref())
        .await
        .expect("seed row");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_cannot_adopt_a_disabled_members_table() {
    let database = DisposableDb::empty("ownership_adopt").await;
    let pool = database.test_pool().await;
    install(&pool, vec![ledger(), notes_owner()])
        .await
        .expect("owner installs");
    seed_note(&pool).await;
    assert_eq!(
        recorded_owner(&pool, "owned_notes").await.as_deref(),
        Some("notes_owner")
    );

    let claimant = unit("claimant", vec![table("owned_notes")], vec![]);
    let err = install(&pool, vec![ledger(), claimant])
        .await
        .expect_err("a table recorded to a disabled member is refused");
    match err {
        LoaderError::DuplicateTableOwner {
            table,
            extension_a,
            extension_b,
        } => {
            assert_eq!(table, "owned_notes");
            assert_eq!(extension_a.as_str(), "notes_owner");
            assert_eq!(extension_b.as_str(), "claimant");
        },
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        recorded_owner(&pool, "owned_notes").await.as_deref(),
        Some("notes_owner")
    );
    assert_eq!(note_count(&pool).await, 1, "the owner's rows are untouched");

    install(&pool, vec![ledger(), notes_owner()])
        .await
        .expect("the recorded owner still declares its table");

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audit_keeps_a_disabled_members_table_apart_from_residue() {
    let database = DisposableDb::empty("ownership_audit").await;
    let pool = database.test_pool().await;
    install(&pool, vec![ledger(), notes_owner()])
        .await
        .expect("owner installs");
    sqlx::query("CREATE TABLE stray_residue (id BIGINT)")
        .execute(pool.write_pool().as_ref())
        .await
        .expect("stray table");

    let peer = unit("peer", vec![table("peer_items")], vec![]);
    let report = install(&pool, vec![ledger(), peer])
        .await
        .expect("peer installs");

    let absent: Vec<(&str, &str)> = report
        .residue
        .absent_member_tables
        .iter()
        .map(|t| (t.owner.as_str(), t.table.as_str()))
        .collect();
    assert_eq!(absent, [("notes_owner", "owned_notes")]);
    let undeclared: Vec<&str> = report
        .residue
        .undeclared_tables
        .iter()
        .map(|t| t.table.as_str())
        .collect();
    assert!(
        undeclared.contains(&"stray_residue"),
        "a truly undeclared table is reported: {undeclared:?}"
    );
    assert!(
        !undeclared.contains(&"owned_notes"),
        "a disabled member's table is not residue"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_re_enabled_after_a_peers_migrations_keeps_its_table_and_rows() {
    let database = DisposableDb::empty("ownership_reenable").await;
    let pool = database.test_pool().await;
    install(&pool, vec![ledger(), notes_owner()])
        .await
        .expect("owner installs");
    seed_note(&pool).await;

    let first = Migration::new(
        1,
        "peer_one",
        "ALTER TABLE peer_items ADD COLUMN IF NOT EXISTS a TEXT;",
    );
    let second = Migration::new(
        2,
        "peer_two",
        "ALTER TABLE peer_items ADD COLUMN IF NOT EXISTS b TEXT;",
    );
    install(
        &pool,
        vec![
            ledger(),
            unit("peer", vec![table("peer_items")], vec![first.clone()]),
        ],
    )
    .await
    .expect("peer installs while the owner is disabled");
    install(
        &pool,
        vec![
            ledger(),
            unit("peer", vec![table("peer_items")], vec![first, second]),
        ],
    )
    .await
    .expect("peer migrates while the owner is disabled");

    let report = install(&pool, vec![ledger(), notes_owner()])
        .await
        .expect("the owner re-enables");
    assert_eq!(
        recorded_owner(&pool, "owned_notes").await.as_deref(),
        Some("notes_owner")
    );
    assert_eq!(
        recorded_owner(&pool, "peer_items").await.as_deref(),
        Some("peer")
    );
    assert_eq!(
        note_count(&pool).await,
        1,
        "the owner's rows survive its absence"
    );
    assert!(
        report
            .residue
            .absent_member_tables
            .iter()
            .any(|t| t.owner.as_str() == "peer" && t.table == "peer_items"),
        "the peer, now absent, keeps its table"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ownership_is_released_only_when_the_owner_drops_its_table() {
    let database = DisposableDb::empty("ownership_release").await;
    let pool = database.test_pool().await;
    let keep = Migration::new(1, "notes_keep", "SELECT 1;");
    install(
        &pool,
        vec![
            ledger(),
            unit(
                "notes_owner",
                vec![table("owned_notes")],
                vec![keep.clone()],
            ),
        ],
    )
    .await
    .expect("owner installs");

    let drop_table = Migration::new(2, "notes_drop", "DROP TABLE IF EXISTS owned_notes;");
    install(
        &pool,
        vec![
            ledger(),
            unit("notes_owner", vec![], vec![keep, drop_table]),
        ],
    )
    .await
    .expect("owner drops its table");
    assert_eq!(recorded_owner(&pool, "owned_notes").await, None);

    install(
        &pool,
        vec![
            ledger(),
            unit("claimant", vec![table("owned_notes")], vec![]),
        ],
    )
    .await
    .expect("a released name may be declared again");
    assert_eq!(
        recorded_owner(&pool, "owned_notes").await.as_deref(),
        Some("claimant")
    );

    drop(pool);
    database.drop_now().await;
}
