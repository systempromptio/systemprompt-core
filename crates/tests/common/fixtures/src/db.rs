//! Integration-test database helpers.
//!
//! Tests that need a real Postgres connection use [`fixture_db_pool`] against
//! the URL exposed via `DATABASE_URL`. The caller is responsible for ensuring
//! the database itself exists and has been migrated (the
//! `systemprompt-test-migrate` binary handles the latter).

use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_database::{Database, DbPool};

/// Resolve the integration-test database URL from the environment.
///
/// Reads `.env` if present, then `DATABASE_URL`. Returns an error with a
/// human-readable hint if unset — DB-backed tests should `?` on this rather
/// than panicking.
pub fn fixture_database_url() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set for DB-backed integration tests"))
}

/// Open a `DbPool` against the supplied URL.
pub async fn fixture_db_pool(url: &str) -> Result<DbPool> {
    let database = Database::new_postgres(url)
        .await
        .context("failed to connect to the integration-test Postgres instance")?;
    Ok(Arc::new(database))
}
