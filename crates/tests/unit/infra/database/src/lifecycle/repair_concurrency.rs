use std::time::Duration;

use systemprompt_database::{
    BOOTSTRAP_ADVISORY_LOCK_KEY, BootstrapLockGuard, DatabaseProvider, MigrationService,
    PostgresProvider,
};
use systemprompt_extension::{Extension, ExtensionMetadata, Migration, SchemaDefinition};
use systemprompt_test_fixtures::DisposableDb;

struct Ext {
    sql: &'static str,
}

impl Extension for Ext {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "repair_cc_ext",
            name: "repair-concurrency",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(1, "counted", self.sql)]
    }
}

const ORIGINAL: &str = "CREATE TABLE IF NOT EXISTS repair_cc_t (n INT NOT NULL);";
const EDITED: &str =
    "CREATE TABLE IF NOT EXISTS repair_cc_t (n INT NOT NULL); INSERT INTO repair_cc_t VALUES (1);";

async fn waiters(provider: &PostgresProvider) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_locks l JOIN pg_database d ON d.oid = l.database \
         WHERE l.locktype = 'advisory' AND NOT l.granted AND d.datname = current_database() \
         AND l.classid = (($1::bigint >> 32) & 4294967295)::oid \
         AND l.objid = ($1::bigint & 4294967295)::oid AND l.objsubid = 1",
    )
    .bind(BOOTSTRAP_ADVISORY_LOCK_KEY)
    .fetch_one(provider.get_postgres_pool().as_ref())
    .await
    .expect("pg_locks")
}

async fn waiting_sessions(provider: &PostgresProvider) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_stat_activity WHERE datname = current_database() \
         AND query LIKE '%pg_try_advisory_lock%' AND pid <> pg_backend_pid() \
         AND pid NOT IN (SELECT pid FROM pg_locks WHERE locktype = 'advisory' AND granted)",
    )
    .fetch_one(provider.get_postgres_pool().as_ref())
    .await
    .expect("pg_stat_activity")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_repairs_reapply_a_drifted_migration_exactly_once() {
    let database = DisposableDb::with_schema("repair_concurrency").await;
    let holder_db = database.test_pool().await;
    let holder = PostgresProvider::from_pool(holder_db.write_pool());
    MigrationService::new(&holder)
        .run_pending_migrations(&Ext { sql: ORIGINAL })
        .await
        .expect("apply original");

    let guard = BootstrapLockGuard::acquire(&holder)
        .await
        .expect("hold the lock");

    let mut tasks = Vec::new();
    for _ in 0..2 {
        let pool = database.test_pool().await;
        tasks.push(tokio::spawn(async move {
            let provider = PostgresProvider::from_pool(pool.write_pool());
            MigrationService::new(&provider)
                .repair_drift(&Ext { sql: EDITED })
                .await
                .map(|r| r.reapplied)
        }));
    }

    let mut settled = false;
    for _ in 0..200 {
        if waiters(&holder).await + waiting_sessions(&holder).await >= 2 {
            settled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        settled,
        "both repairs must be waiting on the bootstrap lock"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    guard.release().await;

    let mut reapplied = Vec::new();
    for task in tasks {
        reapplied.push(task.await.expect("join").expect("repair succeeds"));
    }
    reapplied.sort_unstable();
    assert_eq!(
        reapplied,
        vec![0, 1],
        "exactly one repair re-applies the drift"
    );

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repair_cc_t")
        .fetch_one(holder.get_postgres_pool().as_ref())
        .await
        .expect("count");
    assert_eq!(rows, 1, "the drifted migration body ran once");

    drop(holder_db);
    database.drop_now().await;
}
