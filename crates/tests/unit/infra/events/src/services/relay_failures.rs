//! Relay installation and outbox failure-path tests.
//!
//! Each test runs in its own nextest process, so the process-global
//! `OUTBOX_REPO` `OnceLock` starts empty and each test controls which pool
//! (live, closed, or duplicated) the relay is installed with.

use systemprompt_database::DbPool;
use systemprompt_events::{ANALYTICS_BROADCASTER, Broadcaster, EventRouter};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::{
    closed_db_pool, fixture_database_url, fixture_db_pool, unique_user_id,
};

async fn pool() -> Option<sqlx::PgPool> {
    let url = fixture_database_url().ok()?;
    let db: DbPool = fixture_db_pool(&url).await.ok()?;
    let arc = db.pool_arc().ok()?;
    let pool = (*arc).clone();
    sqlx::query("SELECT 1 FROM event_outbox LIMIT 0")
        .execute(&pool)
        .await
        .ok()?;
    Some(pool)
}

async fn cleanup(pool: &sqlx::PgPool, user: &UserId) {
    let _ = sqlx::query("DELETE FROM event_outbox WHERE user_id = $1")
        .bind(user.as_str())
        .execute(pool)
        .await;
}

#[tokio::test]
async fn install_relay_second_call_is_ignored_and_routing_persists_one_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let user = unique_user_id("relay-idempotent");

    EventRouter::install_relay(pool.clone());
    EventRouter::install_relay(pool.clone());
    EventRouter::route_analytics(&user, AnalyticsEventBuilder::heartbeat()).await;

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM event_outbox WHERE user_id = $1")
        .bind(user.as_str())
        .fetch_one(&pool)
        .await
        .expect("counting outbox rows must succeed");

    cleanup(&pool, &user).await;

    assert_eq!(
        count.0, 1,
        "a double install_relay must not duplicate the outbox append for a single route"
    );
}

#[tokio::test]
async fn outbox_insert_failure_does_not_block_local_delivery() {
    let db = closed_db_pool().await;
    let closed = (*db
        .pool_arc()
        .expect("closed fixture pool must expose a pg pool"))
    .clone();
    EventRouter::install_relay(closed);

    let user = unique_user_id("relay-insert-fail");
    let conn = ConnectionId::new("relay-insert-fail-conn");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let count = EventRouter::route_analytics(&user, AnalyticsEventBuilder::heartbeat()).await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;

    assert_eq!(
        count, 1,
        "a failed outbox insert must not prevent local broadcast delivery"
    );
    assert!(
        rx.try_recv().is_ok(),
        "the local subscriber must still receive the event when the outbox pool is down"
    );
}
