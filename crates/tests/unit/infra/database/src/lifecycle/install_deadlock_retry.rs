//! A schema phase that deadlocks with a live writer is rolled back and
//! retried rather than failing the install. The phase re-creates triggers on
//! two tables in the order `a` then `b`; a writer holds a row lock on `b` and,
//! once the phase waits on `b`, writes `a`. Postgres breaks the cycle by
//! aborting the session that waited first (the phase), so the writer's insert
//! succeeds and the install only succeeds if the phase retried.

use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_extension::SchemaDefinition;
use systemprompt_test_fixtures::install_extension_schemas_with_config;

use super::installation::{StubExtension, drop_table, provider_and_db, registry_with, unique_id};

async fn run(db: &DbPool, sql: String) {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(&*db.write_pool())
        .await
        .expect("setup statement");
}

async fn waits_on(db: &DbPool, table: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pg_locks l JOIN pg_class c ON c.oid = l.relation WHERE \
         c.relname = $1 AND NOT l.granted)",
    )
    .bind(table)
    .fetch_one(&*db.write_pool())
    .await
    .expect("read pg_locks")
}

async fn trigger_count(db: &DbPool, names: [&str; 2]) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pg_trigger WHERE tgname = ANY($1)")
        .bind(names.map(str::to_owned).to_vec())
        .fetch_one(&*db.write_pool())
        .await
        .expect("count triggers")
}

#[tokio::test]
async fn a_phase_that_deadlocks_with_a_writer_is_retried() {
    let (provider, db) = provider_and_db().await;
    let a = unique_id("dl_a");
    let b = unique_id("dl_b");
    let function = unique_id("dl_fn");
    let (trigger_a, trigger_b) = (unique_id("dl_ta"), unique_id("dl_tb"));
    run(&db, format!("CREATE TABLE \"{a}\" (id BIGINT PRIMARY KEY)")).await;
    run(&db, format!("CREATE TABLE \"{b}\" (id BIGINT PRIMARY KEY)")).await;
    run(
        &db,
        format!(
            "CREATE FUNCTION \"{function}\"() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN \
             RETURN NEW; END $$"
        ),
    )
    .await;

    let ext = StubExtension {
        id: unique_id("ext_deadlock"),
        schemas: vec![
            SchemaDefinition::new(
                a,
                format!(
                    "CREATE TABLE IF NOT EXISTS \"{a}\" (id BIGINT PRIMARY KEY);\nDROP TRIGGER IF \
                     EXISTS \"{trigger_a}\" ON \"{a}\";\nCREATE TRIGGER \"{trigger_a}\" BEFORE \
                     INSERT ON \"{a}\" FOR EACH ROW EXECUTE FUNCTION \"{function}\"();"
                ),
            ),
            SchemaDefinition::new(
                b,
                format!(
                    "CREATE TABLE IF NOT EXISTS \"{b}\" (id BIGINT PRIMARY KEY);\nDROP TRIGGER IF \
                     EXISTS \"{trigger_b}\" ON \"{b}\";\nCREATE TRIGGER \"{trigger_b}\" BEFORE \
                     INSERT ON \"{b}\" FOR EACH ROW EXECUTE FUNCTION \"{function}\"();"
                ),
            ),
        ],
        seeds: vec![],
        migrations: vec![],
    };
    let registry = registry_with(ext);

    let mut writer = db.write_pool().begin().await.expect("writer transaction");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO \"{b}\" (id) VALUES (1)"
    )))
    .execute(&mut *writer)
    .await
    .expect("writer holds a row lock on b");

    let install = tokio::spawn(async move {
        install_extension_schemas_with_config(&registry, &provider, &[]).await
    });

    let mut blocked = false;
    for _ in 0..200 {
        if waits_on(&db, b).await {
            blocked = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(blocked, "the phase waits on b while holding a");
    tokio::time::sleep(Duration::from_millis(200)).await;

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO \"{a}\" (id) VALUES (1)"
    )))
    .execute(&mut *writer)
    .await
    .expect("the phase, not the writer, is the deadlock victim");
    writer.commit().await.expect("writer commits");

    let report = tokio::time::timeout(Duration::from_secs(60), install)
        .await
        .expect("the install finishes")
        .expect("install task");
    report.expect("the deadlocked phase was retried and succeeded");
    assert_eq!(trigger_count(&db, [trigger_a, trigger_b]).await, 2);

    drop_table(&db, a).await;
    drop_table(&db, b).await;
    run(&db, format!("DROP FUNCTION IF EXISTS \"{function}\"()")).await;
}
