//! Integration-test database helpers.
//!
//! A DB-backed test gets its database from [`test_database_url`],
//! [`test_db_pool`] or [`test_pg_pool`]. All three panic when `DATABASE_URL`
//! is unset or the server refuses the connection: a test whose database is
//! missing fails, it does not skip. A skipped test reports the same green as
//! one that ran, so an unprovisioned run would be indistinguishable from a
//! passing one. Every supported entry point provides the database: CI sets
//! `DATABASE_URL` for each shard and `just test-shard <group>` does the same
//! locally. The caller is responsible for the database having been migrated
//! (the `systemprompt-test-migrate` binary handles that).
//!
//! [`fixture_database_url`] and [`db_pool_or_skip!`](crate::db_pool_or_skip)
//! are the older skip-on-missing API, kept only until their call sites move to
//! the panicking helpers above.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use systemprompt_database::{Database, DbPool, PoolConfig};

const MISSING_DATABASE_URL: &str =
    "DATABASE_URL is not set — run DB tests through `just test-shard <group>`";

pub fn test_database_url() -> String {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .expect(MISSING_DATABASE_URL)
}

pub async fn test_db_pool() -> DbPool {
    let url = test_database_url();
    fixture_db_pool(&url)
        .await
        .unwrap_or_else(|e| panic!("DATABASE_URL is set but unusable: {e:#}"))
}

pub async fn test_pg_pool() -> sqlx::PgPool {
    test_db_pool().await.write_pool().as_ref().clone()
}

pub fn fixture_database_url() -> Result<String> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty());
    match url {
        Some(url) => Ok(url),
        None => {
            crate::skip::skip_or_panic("DATABASE_URL", "DB-backed tests need a live Postgres");
            Err(anyhow::anyhow!(
                "DATABASE_URL must be set for DB-backed integration tests"
            ))
        },
    }
}

pub fn fixture_database_url_opt() -> Option<String> {
    fixture_database_url().ok()
}

// Why: the only sanctioned way for a DB-backed test to give up. The `return`
// is unreachable under CI -- `fixture_database_url` panics first -- so the
// early exit is a developer-machine convenience, not a hole in the tier.
#[macro_export]
macro_rules! db_pool_or_skip {
    () => {{
        let Some(url) = $crate::db::fixture_database_url_opt() else {
            return; // skip-ok: fixture_database_url panics under CI
        };
        let pool = $crate::db::fixture_db_pool(&url)
            .await
            .expect("DATABASE_URL is set, so connecting to it must succeed");
        (pool, url)
    }};
}

// Connection ceiling for a single test's pool.
//
// The budget is `RUST_TEST_THREADS` (8, set in this workspace's cargo config)
// times this value against Postgres `max_connections` of 100. Connections open
// on demand (`min_connections` is 0) and close with the pool, so the ceiling
// only has to cover one test's concurrent queries.
const FIXTURE_POOL_MAX_CONNECTIONS: u32 = 8;

// Idle connections are returned to the server promptly rather than parked for
// the default five minutes: a test's pool outlives the test by however long the
// binary runs, and parked connections from finished tests are what exhausts the
// server mid-run.
const FIXTURE_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

// A pool handle that never opens a socket: the connection is lazy and no
// query ever runs through it. Fake `DatabaseProvider`s that script every call
// use it to satisfy `get_postgres_pool` while staying DB-free.
pub fn lazy_pg_pool() -> Arc<sqlx::PgPool> {
    let pool = sqlx::PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:1/fake")
        .expect("lazy pool construction is infallible for a well-formed URL");
    Arc::new(pool)
}

// A `DbPool` whose every acquire fails deterministically.
//
// The sqlx pool is created lazily (no connection is ever established) and
// closed immediately, so any query through it returns `PoolClosed`. Error-
// propagation tests use this to drive a repository's `.map_err` arm without
// breaking a live connection.
pub async fn closed_db_pool() -> DbPool {
    let pool = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed")
        .expect("lazy pool construction is infallible for a well-formed URL");
    pool.close().await;
    Arc::new(Database::from_pools(Arc::new(pool), None))
}

/// The pool belongs to the calling test: a sqlx connection registers its socket
/// with the reactor of the runtime that opened it, so one shared across
/// `#[tokio::test]` runtimes hands a later test a connection whose runtime is
/// gone ("Tokio 1.x context ... is being shutdown"). Callers that need the same
/// pool twice should clone the handle rather than call this again.
pub async fn fixture_db_pool(url: &str) -> Result<DbPool> {
    let cfg = PoolConfig {
        max_connections: FIXTURE_POOL_MAX_CONNECTIONS,
        idle_timeout: FIXTURE_POOL_IDLE_TIMEOUT,
        ..PoolConfig::default()
    };
    Database::connect(url, None, &cfg)
        .await
        .map(Arc::new)
        .context("failed to connect to the integration-test Postgres instance")
}
