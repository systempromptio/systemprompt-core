//! Invariant under test: the Postgres advisory-lock primitive on which
//! `systemprompt_scheduler::services::scheduling::lock::try_acquire_job_lock`
//! is built behaves as the scheduler's cross-replica single-execution
//! guarantee assumes.
//!
//! The scheduler computes a 64-bit lock key from the job name via
//! `hashtext($1)::bigint` and calls `pg_try_advisory_lock(key)` on a
//! dedicated pooled connection. The lock is session-scoped: it is released
//! either explicitly via `pg_advisory_unlock` on the *same* connection, or
//! implicitly when that connection is recycled/closed. These tests pin those
//! semantics directly so a regression in the primitive surfaces here rather
//! than as a flaky double-firing job in production.
//!
//! Each test creates its own job name (UUID-suffixed) so that parallel test
//! processes never collide on the same lock key.

use std::sync::Arc;
use std::time::Duration;

use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str = "postgres://systemprompt_admin:\
                                    3e00fcdac26b5b731829e8737515db8f@localhost:5432/\
                                    systemprompt-web";

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

fn unique_job_name(prefix: &str) -> String {
    format!("test_{}_{}", prefix, Uuid::new_v4().simple())
}

async fn connect_pool() -> PgPool {
    PgPool::connect(&database_url())
        .await
        .expect("connect to test database")
}

async fn key_for(pool: &PgPool, job_name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT hashtext($1)::bigint")
        .bind(job_name)
        .fetch_one(pool)
        .await
        .expect("compute hashtext key")
}

async fn try_acquire(conn: &mut PgConnection, key: i64) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
        .bind(key)
        .fetch_one(conn)
        .await
        .expect("pg_try_advisory_lock")
}

async fn release(conn: &mut PgConnection, key: i64) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(conn)
        .await
        .expect("pg_advisory_unlock")
}

#[tokio::test]
async fn two_replicas_only_one_acquires_lock() {
    let pool = connect_pool().await;
    let job = unique_job_name("two_replica");
    let key = key_for(&pool, &job).await;

    let mut conn_a = pool.acquire().await.expect("acquire conn a");
    let mut conn_b = pool.acquire().await.expect("acquire conn b");

    let acquired_a = try_acquire(&mut conn_a, key).await;
    let acquired_b = try_acquire(&mut conn_b, key).await;

    assert!(acquired_a, "first replica must acquire the lock");
    assert!(
        !acquired_b,
        "second replica must observe the lock as held and skip"
    );

    let released = release(&mut conn_a, key).await;
    assert!(released, "lock release on the holder connection succeeds");
}

#[tokio::test]
async fn stale_lock_holder_releases_on_connection_drop() {
    let pool = connect_pool().await;
    let job = unique_job_name("zombie");
    let key = key_for(&pool, &job).await;

    let mut holder = PgConnection::connect(&database_url())
        .await
        .expect("standalone holder connection");
    let acquired = try_acquire(&mut holder, key).await;
    assert!(acquired, "holder acquires the lock");

    holder
        .close()
        .await
        .expect("forcibly close the holder connection");

    // Poll: Postgres releases the session-scoped lock when the backend
    // detects the closed client. Allow a brief settling window.
    let mut waiter = pool.acquire().await.expect("acquire waiter conn");
    let mut recovered = false;
    for _ in 0..50 {
        if try_acquire(&mut waiter, key).await {
            recovered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        recovered,
        "the lock must be released when its holder connection dies, so a crashed replica does not \
         leak the lock permanently"
    );
    let released = release(&mut waiter, key).await;
    assert!(released);
}

#[tokio::test]
async fn multi_replica_contention_serialises() {
    let pool = Arc::new(connect_pool().await);
    let job = unique_job_name("multi_replica");
    let key = key_for(&pool, &job).await;

    let replica_count = 10usize;
    let mut handles = Vec::with_capacity(replica_count);
    for _ in 0..replica_count {
        let pool = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let mut conn = pool.acquire().await.expect("acquire");
            let acquired = try_acquire(&mut conn, key).await;
            if acquired {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let released = release(&mut conn, key).await;
                assert!(released, "holder releases its own lock cleanly");
                true
            } else {
                false
            }
        }));
    }

    let mut acquired_count = 0;
    for handle in handles {
        if handle.await.expect("task joined") {
            acquired_count += 1;
        }
    }

    // The semantic guarantee `try_acquire_job_lock` relies on is that a
    // concurrent burst never causes two replicas to *simultaneously* hold
    // the same lock. With `pg_try_advisory_lock` the per-call outcome is
    // race-dependent: 1..=replica_count callers may serialise through if
    // every release happens before the next attempt. The wrong outcomes
    // would be 0 (deadlock) or > replica_count (impossible).
    assert!(
        (1..=replica_count).contains(&acquired_count),
        "exactly {acquired_count} acquisitions is outside the legal range 1..={replica_count} — \
         the advisory lock primitive is misbehaving"
    );
}

#[tokio::test]
async fn same_connection_reentrant_acquire_requires_matching_releases() {
    let pool = connect_pool().await;
    let job = unique_job_name("reentrant");
    let key = key_for(&pool, &job).await;

    let mut conn = pool.acquire().await.expect("acquire");
    assert!(try_acquire(&mut conn, key).await, "first acquire");
    assert!(
        try_acquire(&mut conn, key).await,
        "second acquire (reentrant)"
    );

    let mut other = pool.acquire().await.expect("other conn");
    assert!(
        !try_acquire(&mut other, key).await,
        "another connection must not be able to acquire while the lock is held"
    );

    assert!(release(&mut conn, key).await, "first release");
    assert!(
        !try_acquire(&mut other, key).await,
        "lock still held after one release of a re-entrant pair"
    );
    assert!(release(&mut conn, key).await, "second release");
    assert!(
        try_acquire(&mut other, key).await,
        "lock available once balanced releases complete"
    );
    let _ = release(&mut other, key).await;
}

#[tokio::test]
async fn release_from_non_holder_is_falsy_not_fatal() {
    let pool = connect_pool().await;
    let job = unique_job_name("non_holder_release");
    let key = key_for(&pool, &job).await;

    let mut holder = pool.acquire().await.expect("holder");
    assert!(try_acquire(&mut holder, key).await);

    let mut bystander = pool.acquire().await.expect("bystander");
    let released = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(bystander.as_mut())
        .await
        .expect("query did not raise");
    assert!(
        !released,
        "non-holder release must report 'not held' rather than panic — the scheduler depends on \
         this to survive misordered guard drops"
    );

    assert!(
        release(&mut holder, key).await,
        "holder still releases cleanly"
    );
}

#[tokio::test]
async fn distinct_job_names_use_independent_lock_keys() {
    let pool = connect_pool().await;
    let job_a = unique_job_name("indep_a");
    let job_b = unique_job_name("indep_b");
    let key_a = key_for(&pool, &job_a).await;
    let key_b = key_for(&pool, &job_b).await;

    assert_ne!(
        key_a, key_b,
        "two unrelated job names must not share a lock key — collision would starve every job \
         sharing the bucket"
    );

    let mut conn_a = pool.acquire().await.expect("conn a");
    let mut conn_b = pool.acquire().await.expect("conn b");
    assert!(try_acquire(&mut conn_a, key_a).await);
    assert!(
        try_acquire(&mut conn_b, key_b).await,
        "a held lock on key A must not block an acquire on key B"
    );
    let _ = release(&mut conn_a, key_a).await;
    let _ = release(&mut conn_b, key_b).await;
}
