//! Integration tests for the `systemprompt-events` cross-replica relay.
//!
//! These tests require a running PostgreSQL database. Set the `DATABASE_URL`
//! environment variable before running.

#[cfg(test)]
mod cross_replica;

use sqlx::PgPool;
use std::sync::Arc;

pub fn fixture_database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL required for PostgreSQL integration tests")
}

pub fn unique_user_id(prefix: &str) -> systemprompt_identifiers::UserId {
    systemprompt_identifiers::UserId::new(format!(
        "{prefix}_{}",
        systemprompt_identifiers::ConnectionId::generate()
    ))
}

pub async fn setup_test_pool() -> Arc<PgPool> {
    let url = fixture_database_url();
    let pool = PgPool::connect(&url)
        .await
        .expect("failed to connect to test database");

    Arc::new(pool)
}

pub async fn ensure_event_outbox(pool: &PgPool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS event_outbox (id TEXT PRIMARY KEY, channel TEXT NOT NULL, \
         user_id TEXT NOT NULL, payload JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT \
         now())",
    )
    .execute(pool)
    .await
    .expect("failed to ensure event_outbox table");

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_event_outbox_created_at ON event_outbox(created_at)",
    )
    .execute(pool)
    .await
    .expect("failed to ensure event_outbox index");
}
