//! Invariant under test: the generic projection queue drains every queued
//! key exactly once on success, puts a failed batch back one attempt dearer,
//! and refuses a projector name that is not a bare identifier.

use std::time::Duration;

use sqlx::PgPool;
use systemprompt_database::projection::{depth, depths, drain, drain_with_sql, queue};
use systemprompt_test_fixtures::test_database_url;
use uuid::Uuid;

async fn connect_pool() -> PgPool {
    PgPool::connect(&test_database_url())
        .await
        .expect("connect to test database")
}

fn unique(prefix: &str) -> String {
    format!("test_{prefix}_{}", Uuid::new_v4().simple())
}

fn keys(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("k{i:03}")).collect()
}

async fn attempts(pool: &PgPool, projection: &str) -> Vec<i32> {
    sqlx::query_scalar::<_, i32>(
        "SELECT attempts FROM projection_backlog WHERE projection = $1 ORDER BY key",
    )
    .bind(projection)
    .fetch_all(pool)
    .await
    .expect("read attempts")
}

async fn purge(pool: &PgPool, projection: &str) {
    sqlx::query("DELETE FROM projection_backlog WHERE projection = $1")
        .bind(projection)
        .execute(pool)
        .await
        .expect("purge backlog");
}

#[tokio::test]
async fn queue_counts_only_new_keys_and_depth_reflects_them() {
    let pool = connect_pool().await;
    let projection = unique("queue");

    let first = queue(&pool, &projection, &keys(3)).await.expect("queue");
    let again = queue(&pool, &projection, &keys(4)).await.expect("requeue");

    assert_eq!(first, 3);
    assert_eq!(again, 1, "already queued keys are not counted twice");
    assert_eq!(depth(&pool, &projection).await.expect("depth"), 4);
    let all = depths(&pool).await.expect("depths");
    assert!(all.iter().any(|(p, d)| p == &projection && *d == 4));

    purge(&pool, &projection).await;
}

#[tokio::test]
async fn drain_refreshes_every_key_in_batches_and_empties_the_queue() {
    let pool = connect_pool().await;
    let projection = unique("drain_ok");
    queue(&pool, &projection, &keys(5)).await.expect("queue");

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = std::sync::Arc::clone(&seen);
    let report = drain(&pool, &projection, 2, Duration::from_secs(30), move |_conn, batch| {
        let sink = std::sync::Arc::clone(&sink);
        Box::pin(async move {
            let n = u64::try_from(batch.len()).expect("batch fits");
            sink.lock().expect("lock").extend(batch);
            Ok(n)
        })
    })
    .await
    .expect("drain");

    assert_eq!(report.keys, 5);
    assert_eq!(report.batches, 3);
    assert_eq!(report.remaining, 0);
    assert_eq!(report.requeued, 0);
    let mut seen = seen.lock().expect("lock").clone();
    seen.sort();
    assert_eq!(seen, keys(5));
    assert_eq!(depth(&pool, &projection).await.expect("depth"), 0);
}

#[tokio::test]
async fn drain_on_empty_queue_reports_nothing() {
    let pool = connect_pool().await;
    let projection = unique("drain_empty");

    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&calls);
    let report = drain(&pool, &projection, 10, Duration::from_secs(5), move |_conn, _batch| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { Ok(0) })
    })
    .await
    .expect("drain");

    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);

    assert_eq!(report.keys, 0);
    assert_eq!(report.batches, 0);
    assert_eq!(report.remaining, 0);
}

#[tokio::test]
async fn failed_refresh_rolls_back_and_requeues_one_attempt_dearer() {
    let pool = connect_pool().await;
    let projection = unique("drain_fail");
    queue(&pool, &projection, &keys(3)).await.expect("queue");

    let report = drain(&pool, &projection, 10, Duration::from_secs(30), |conn, _batch| {
        Box::pin(async move {
            sqlx::query("SELECT 1 / 0").execute(&mut *conn).await?;
            Ok(0)
        })
    })
    .await
    .expect("drain reports, it does not fail");

    assert_eq!(report.keys, 0);
    assert_eq!(report.batches, 0);
    assert_eq!(report.requeued, 3);
    assert_eq!(report.remaining, 3);
    assert_eq!(attempts(&pool, &projection).await, vec![1, 1, 1]);

    purge(&pool, &projection).await;
}

#[tokio::test]
async fn drain_with_sql_calls_the_named_projector() {
    let pool = connect_pool().await;
    let projection = unique("drain_sql");
    let projector = unique("refresh_fn");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {projector}(keys TEXT[]) RETURNS BIGINT LANGUAGE sql AS \
         $$ SELECT COALESCE(array_length(keys, 1), 0)::BIGINT $$"
    )))
    .execute(&pool)
    .await
    .expect("create projector");
    queue(&pool, &projection, &keys(4)).await.expect("queue");

    let report = drain_with_sql(&pool, &projection, 3, Duration::from_secs(30), &projector).await;

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP FUNCTION IF EXISTS {projector}(TEXT[])"
    )))
    .execute(&pool)
    .await
    .expect("drop projector");
    let report = report.expect("drain_with_sql");
    assert_eq!(report.keys, 4);
    assert_eq!(report.batches, 2);
    assert_eq!(report.remaining, 0);
}

#[tokio::test]
async fn drain_with_sql_rejects_a_non_identifier_projector() {
    let pool = connect_pool().await;
    let projection = unique("drain_bad");
    queue(&pool, &projection, &keys(1)).await.expect("queue");

    for bad in ["", "Refresh", "1refresh", "refresh; DROP TABLE x", "a.b", "refresh-x"] {
        let err = drain_with_sql(&pool, &projection, 1, Duration::from_secs(5), bad)
            .await
            .expect_err("bad projector refused");
        let message = err.to_string();
        assert!(
            message.contains("invalid argument projector"),
            "{bad:?}: {message}"
        );
    }
    assert_eq!(depth(&pool, &projection).await.expect("depth"), 1, "queue untouched");

    purge(&pool, &projection).await;
}
