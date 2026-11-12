use std::time::Duration;

use systemprompt_database::{DbPool, MigrationConfig, install_extension_schemas_full};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_test_fixtures::DisposableDb;

async fn install(pool: &DbPool) -> Result<(), systemprompt_extension::LoaderError> {
    install_extension_schemas_full(
        &ExtensionRegistry::discover().expect("registry"),
        pool.write(),
        &[],
        MigrationConfig::default(),
    )
    .await
    .map(|_| ())
}

async fn triggered_tables(pool: &sqlx::PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT DISTINCT c.relname::text FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE NOT t.tgisinternal AND n.nspname = 'public' ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("triggered tables")
}

async fn stored_fingerprint(pool: &sqlx::PgPool) -> Option<String> {
    sqlx::query_scalar("SELECT fingerprint FROM schema_install_fingerprint WHERE singleton")
        .fetch_optional(pool)
        .await
        .expect("fingerprint")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_settled_reinstall_takes_no_lock_on_tables_writers_hold() {
    let database = DisposableDb::with_schema("settled_no_ddl_locks").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    assert!(
        stored_fingerprint(&raw).await.is_some(),
        "a complete install records its fingerprint"
    );

    let tables = triggered_tables(&raw).await;
    assert!(!tables.is_empty(), "the registry declares triggers");

    let writer_pool = database.test_pool().await;
    let mut writer = writer_pool.write_pool().begin().await.expect("writer tx");
    for table in &tables {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "LOCK TABLE \"{table}\" IN ROW EXCLUSIVE MODE"
        )))
        .execute(&mut *writer)
        .await
        .expect("writer row lock");
    }

    let outcome = tokio::time::timeout(Duration::from_secs(8), install(&pool)).await;
    writer.rollback().await.expect("release writer");
    outcome
        .expect("a settled install must not wait on a writer's row lock")
        .expect("settled install succeeds");

    drop(writer_pool);
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_changed_fingerprint_runs_the_full_install_and_heals_the_schema() {
    let database = DisposableDb::with_schema("settled_stale_fingerprint").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let recorded = stored_fingerprint(&raw).await.expect("recorded");

    let table = triggered_tables(&raw)
        .await
        .into_iter()
        .next()
        .expect("a triggered table");
    let trigger: String = sqlx::query_scalar(
        "SELECT t.tgname::text FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
         WHERE c.relname = $1 AND NOT t.tgisinternal ORDER BY 1 LIMIT 1",
    )
    .bind(&table)
    .fetch_one(raw.as_ref())
    .await
    .expect("trigger");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER \"{trigger}\" ON \"{table}\""
    )))
    .execute(raw.as_ref())
    .await
    .expect("drop trigger");
    sqlx::query("UPDATE schema_install_fingerprint SET fingerprint = 'stale'")
        .execute(raw.as_ref())
        .await
        .expect("stale fingerprint");

    install(&pool).await.expect("full install");

    let restored: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid \
         WHERE c.relname = $1 AND t.tgname = $2)",
    )
    .bind(&table)
    .bind(&trigger)
    .fetch_one(raw.as_ref())
    .await
    .expect("trigger lookup");
    assert!(restored, "the full install re-creates {trigger} on {table}");
    assert_eq!(stored_fingerprint(&raw).await, Some(recorded));

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_owned_table_is_recreated_even_when_the_fingerprint_matches() {
    let database = DisposableDb::with_schema("settled_missing_table").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();

    sqlx::query("DROP TABLE release_stamps CASCADE")
        .execute(raw.as_ref())
        .await
        .expect("drop owned table");

    install(&pool).await.expect("install");

    let present: bool = sqlx::query_scalar("SELECT to_regclass('release_stamps') IS NOT NULL")
        .fetch_one(raw.as_ref())
        .await
        .expect("regclass");
    assert!(
        present,
        "an owned table that is gone forces the full install"
    );

    drop(pool);
    database.drop_now().await;
}
