//! Integration tests for the `systemprompt-events` cross-replica relay.
//!
//! These tests require a running PostgreSQL database. Set the `DATABASE_URL`
//! environment variable before running.

#[cfg(test)]
mod cross_replica;

use sqlx::PgPool;
use std::sync::Arc;

pub fn unique_user_id(prefix: &str) -> systemprompt_identifiers::UserId {
    systemprompt_identifiers::UserId::new(format!(
        "{prefix}_{}",
        systemprompt_identifiers::ConnectionId::generate()
    ))
}

pub async fn setup_test_pool() -> Arc<PgPool> {
    let url = systemprompt_test_fixtures::test_database_url();
    let pool = PgPool::connect(&url)
        .await
        .expect("failed to connect to test database");

    Arc::new(pool)
}
