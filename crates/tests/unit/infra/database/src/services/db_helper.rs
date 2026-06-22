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
use systemprompt_test_fixtures::fixture_database_url;

pub async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let db = Database::from_config_with_write("postgres", &url, None, &cfg)
        .await
        .ok()?;
    Some(Arc::new(db))
}
