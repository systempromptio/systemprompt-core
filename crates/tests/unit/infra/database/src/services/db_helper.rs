//! Shared DB-pool helper for the DB-backed `services` tests.
//!
//! A fresh, small-capacity [`systemprompt_database::Database`] is built per
//! test rather than reusing a process-wide `OnceCell` pool: under `cargo test`
//! every `#[tokio::test]` runs on its own current-thread runtime, and a pool
//! whose background tasks are bound to a since-shut-down runtime surfaces
//! `PoolTimedOut` / "Tokio context shutdown" for later tests. A per-test pool
//! sidesteps that; `min_connections = 0` keeps the live-connection count well
//! under the server limit even when many tests run at once.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{Database, DbPool, PoolConfig};
use systemprompt_test_fixtures::{lazy_pg_pool, test_database_url};

pub async fn test_pool() -> DbPool {
    let url = test_database_url();
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let db = Database::connect(&url, None, &cfg)
        .await
        .expect("connect to the test database");
    Arc::new(db)
}

pub fn lazy_pool() -> Arc<sqlx::PgPool> {
    lazy_pg_pool()
}
