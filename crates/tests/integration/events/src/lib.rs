//! Integration tests for the `systemprompt-events` cross-replica relay.
//!
//! These tests require a running PostgreSQL database. Set the `DATABASE_URL`
//! environment variable before running.

#[cfg(test)]
mod cross_replica;

use std::env;
use std::sync::Arc;
use systemprompt_database::{Database, PgPool};

/// Connects to the test database named by `DATABASE_URL` and returns its
/// Postgres pool, the surface the relay (`EventRouter` + `PostgresEventBridge`)
/// is built on.
pub async fn setup_test_pool() -> Arc<PgPool> {
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");

    let db = Database::new_postgres(&database_url)
        .await
        .expect("failed to connect to test database");

    db.pool_arc()
        .expect("test database is not Postgres-backed")
}

/// Ensures the `event_outbox` table exists. The schema mirrors
/// `crates/infra/events/schema/event_outbox.sql`; a freshly migrated
/// database already has it, and `IF NOT EXISTS` keeps this idempotent.
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
