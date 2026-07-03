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
use tokio::sync::OnceCell;

pub fn fixture_database_url() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set for DB-backed integration tests"))
}

static SHARED_POOL: OnceCell<DbPool> = OnceCell::const_new();

/// Per-process connection ceiling for the shared test pool.
///
/// nextest runs one process per test, so N processes each hold their own
/// `SHARED_POOL`. Postgres `max_connections` is 100 and a shard runs 4 test
/// threads (processes) at a time, so the historical 50-per-pool default could
/// demand ~200 connections and starve acquires past the 30s timeout — the
/// multi-query messaging dispatch tests were the ones that tipped over. Bound
/// each pool so 4 concurrent processes stay well under the server ceiling.
const FIXTURE_POOL_MAX_CONNECTIONS: u32 = 12;

pub async fn fixture_db_pool(url: &str) -> Result<DbPool> {
    let pool = SHARED_POOL
        .get_or_try_init(|| async {
            let cfg = PoolConfig {
                max_connections: FIXTURE_POOL_MAX_CONNECTIONS,
                ..PoolConfig::default()
            };
            Database::from_config_with_write("postgres", url, None, &cfg)
                .await
                .map(Arc::new)
                .context("failed to connect to the integration-test Postgres instance")
        })
        .await?;
    Ok(Arc::clone(pool))
}
