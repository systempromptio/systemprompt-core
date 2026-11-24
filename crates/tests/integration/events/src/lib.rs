//! Integration tests for the `systemprompt-events` cross-replica relay.
//!
//! These tests require a running PostgreSQL database. Set the `DATABASE_URL`
//! environment variable before running.
//!
//! A test that asserts on outbox ledger state works in its own
//! [`OutboxSchema`]: `compact_event_outbox_backlog` and the rollup job fold
//! every consumer's rows in the schema they run in, so on the shared `public`
//! tables a compaction from any other process (a sibling test, a served
//! instance's scheduler) lands between a test's write and its assertion.

#[cfg(test)]
mod cross_replica;

#[cfg(test)]
mod durable;

#[cfg(test)]
mod durable_batch;

#[cfg(test)]
mod claim_throughput;

#[cfg(test)]
mod config_watch_notify;

#[cfg(test)]
mod fact_channel;

#[cfg(test)]
mod outbox_backlog;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

pub fn unique_user_id(prefix: &str) -> systemprompt_identifiers::UserId {
    systemprompt_identifiers::UserId::new(format!(
        "{prefix}_{}",
        systemprompt_identifiers::ConnectionId::generate()
    ))
}

pub async fn setup_test_pool() -> Arc<PgPool> {
    Arc::new(systemprompt_test_fixtures::test_pg_pool().await)
}

// A scratch schema holding the event outbox tables, triggers and functions.
pub struct OutboxSchema {
    admin: PgPool,
    pool: PgPool,
    name: String,
}

impl OutboxSchema {
    pub async fn create() -> Self {
        let admin = systemprompt_test_fixtures::test_pg_pool().await;
        let suffix = systemprompt_identifiers::ConnectionId::generate()
            .to_string()
            .replace('-', "_");
        let name = format!("outbox_test_{suffix}");
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA \"{name}\"")))
            .execute(&admin)
            .await
            .expect("create the scratch outbox schema");
        let pool = Self::connect(
            &name,
            systemprompt_test_fixtures::db::FIXTURE_POOL_MAX_CONNECTIONS,
        )
        .await;
        for ddl in [
            include_str!("../../../../infra/events/schema/event_outbox.sql"),
            include_str!("../../../../infra/events/schema/event_outbox_backlog.sql"),
            include_str!("../../../../infra/events/schema/migrations/004_durable_consumption.sql"),
        ] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
                .execute(&pool)
                .await
                .expect("install the outbox schema");
        }
        Self { admin, pool, name }
    }

    pub async fn pool_of(&self, max_connections: u32) -> PgPool {
        Self::connect(&self.name, max_connections).await
    }

    async fn connect(name: &str, max_connections: u32) -> PgPool {
        let search_path = name.to_owned();
        PgPoolOptions::new()
            .max_connections(max_connections)
            .idle_timeout(systemprompt_test_fixtures::db::FIXTURE_POOL_IDLE_TIMEOUT)
            .after_connect(move |connection, _| {
                let search_path = search_path.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(&search_path)
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SELECT set_config('application_name', $1, false)")
                        .bind(&search_path)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&systemprompt_test_fixtures::test_database_url())
            .await
            .expect("connect to the scratch outbox schema")
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn drop_now(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP SCHEMA \"{}\" CASCADE",
            self.name
        )))
        .execute(&self.admin)
        .await
        .expect("drop the scratch outbox schema");
    }
}
