//! A full install on an established database leaves every
//! `CREATE INDEX IF NOT EXISTS` whose index already exists out of its phase,
//! so it takes no `SHARE` lock on a live table: it completes while a writer
//! holds `ROW EXCLUSIVE` on every table. An index that is missing is still
//! created, and that statement still waits for the writer.

use std::time::Duration;

use systemprompt_database::{DbPool, MigrationConfig, install_admitted_schemas};
use systemprompt_test_fixtures::{DisposableDb, schema_admitted};

async fn full_install(pool: DbPool) -> Result<(), String> {
    sqlx::query("UPDATE schema_install_fingerprint SET fingerprint = 'stale'")
        .execute(pool.write_pool().as_ref())
        .await
        .map_err(|e| e.to_string())?;
    install_admitted_schemas(
        schema_admitted().expect("core members").built(),
        pool.write(),
        MigrationConfig::default(),
    )
    .await
    .map(drop)
    .map_err(|e| e.to_string())
}

async fn hold_row_exclusive_on_every_table(
    raw: &sqlx::PgPool,
) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let tables: String = sqlx::query_scalar(
        "SELECT string_agg(format('%I.%I', n.nspname, c.relname), ', ') FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p')",
    )
    .fetch_one(raw)
    .await
    .expect("table list");
    let mut writer = raw.begin().await.expect("writer");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "LOCK TABLE {tables} IN ROW EXCLUSIVE MODE"
    )))
    .execute(&mut *writer)
    .await
    .expect("writer locks");
    writer
}

async fn installer_waits_for(raw: &sqlx::PgPool, relation: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l \
         WHERE NOT l.granted AND l.relation = to_regclass($1) AND l.mode = 'ShareLock')",
    )
    .bind(relation)
    .fetch_one(raw)
    .await
    .expect("lock waits")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_install_takes_no_share_lock_for_existing_indexes() {
    let database = DisposableDb::with_schema("index_presence_noop").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let indexes_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public'")
            .fetch_one(raw.as_ref())
            .await
            .expect("index count");

    let writer = hold_row_exclusive_on_every_table(&raw).await;
    let outcome = tokio::time::timeout(Duration::from_secs(60), full_install(pool.clone())).await;
    let waited = installer_waits_for(&raw, "documents").await;
    writer.rollback().await.expect("writer rollback");

    assert_eq!(
        outcome.expect("the install does not wait on the writer's tables"),
        Ok(())
    );
    assert!(!waited, "no ShareLock wait on documents");
    let indexes_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public'")
            .fetch_one(raw.as_ref())
            .await
            .expect("index count");
    assert_eq!(indexes_after, indexes_before);

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_missing_index_is_created_and_waits_for_the_writer() {
    let database = DisposableDb::with_schema("index_presence_missing").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let (index, table): (String, String) = sqlx::query_as(
        "SELECT indexname::text, tablename::text FROM pg_indexes \
         WHERE schemaname = 'public' AND tablename = 'documents' \
           AND indexname NOT IN (SELECT conname FROM pg_constraint) \
         ORDER BY indexname LIMIT 1",
    )
    .fetch_one(raw.as_ref())
    .await
    .expect("a declared, non-constraint index on documents");
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP INDEX \"{index}\"")))
        .execute(raw.as_ref())
        .await
        .expect("drop the index");

    let writer = hold_row_exclusive_on_every_table(&raw).await;
    let install = tokio::spawn(full_install(pool.clone()));
    let mut waited = false;
    for _ in 0..200 {
        if installer_waits_for(&raw, &table).await {
            waited = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    writer.rollback().await.expect("writer rollback");
    let outcome = tokio::time::timeout(Duration::from_secs(120), install)
        .await
        .expect("the install finishes once the writer is gone")
        .expect("install task");

    assert!(
        waited,
        "re-creating {index} takes SHARE on {table} and waits"
    );
    assert_eq!(outcome, Ok(()));
    let restored: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = $1)",
    )
    .bind(&index)
    .fetch_one(raw.as_ref())
    .await
    .expect("index lookup");
    assert!(restored, "{index} is created again");

    drop(pool);
    database.drop_now().await;
}
