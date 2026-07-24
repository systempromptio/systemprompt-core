//! Integration-test database helpers.
//!
//! Tests that need a real Postgres connection use [`fixture_db_pool`] against
//! the URL exposed via `DATABASE_URL`. The caller is responsible for ensuring
//! the database itself exists and has been migrated (the
//! `systemprompt-test-migrate` binary handles the latter).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use systemprompt_database::{Database, DbPool, PoolConfig};

pub fn fixture_database_url() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set for DB-backed integration tests"))
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
    Database::from_config_with_write("postgres", url, None, &cfg)
        .await
        .map(Arc::new)
        .context("failed to connect to the integration-test Postgres instance")
}
