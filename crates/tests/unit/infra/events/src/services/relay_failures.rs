//! Router construction and outbox failure-path tests.
//!
//! Each test builds its own [`EventRouter`] over the pool it needs (live,
//! closed, or none), so no test depends on what another installed.

use systemprompt_database::DbPool;
use systemprompt_events::{
    ANALYTICS_BROADCASTER, Broadcaster, EventRouter, RelayError, RelayOutcome,
};
use systemprompt_identifiers::{ConnectionId, InstanceId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::{closed_db_pool, test_db_pool, unique_user_id};

async fn fixture_pool() -> sqlx::PgPool {
    let db: DbPool = test_db_pool().await;
    let arc = db.pool();
    let pool = (*arc).clone();
    sqlx::query("SELECT 1 FROM event_outbox LIMIT 0")
        .execute(&pool)
        .await
        .expect("event_outbox migration prerequisite");
    pool
}

async fn outbox_rows(pool: &sqlx::PgPool, user: &UserId) -> i64 {
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM event_outbox WHERE user_id = $1")
        .bind(user.as_str())
        .fetch_one(pool)
        .await
        .expect("counting outbox rows must succeed");
    count.0
}

async fn cleanup(pool: &sqlx::PgPool, user: &UserId) {
    let _ = sqlx::query("DELETE FROM event_outbox WHERE user_id = $1")
        .bind(user.as_str())
        .execute(pool)
        .await;
}

#[tokio::test]
async fn local_only_router_reports_not_installed_and_writes_no_row() {
    let pool = fixture_pool().await;
    let user = unique_user_id("relay-local-only");

    let outcome = EventRouter::local_only()
        .route_analytics(&user, AnalyticsEventBuilder::heartbeat())
        .await;

    let count = outbox_rows(&pool, &user).await;
    cleanup(&pool, &user).await;

    assert!(
        matches!(outcome.relay, RelayOutcome::NotInstalled),
        "a local-only router must report the relay as not installed, got {:?}",
        outcome.relay
    );
    assert_eq!(
        count, 0,
        "a local-only router must never append to the outbox"
    );
}

#[tokio::test]
async fn routers_over_one_pool_each_persist_exactly_their_own_row() {
    let pool = fixture_pool().await;
    let user = unique_user_id("relay-two-routers");
    let first = EventRouter::with_outbox(pool.clone(), InstanceId::new("relay-origin-a"));
    let second = EventRouter::with_outbox(pool.clone(), InstanceId::new("relay-origin-b"));

    let first_outcome = first
        .route_analytics(&user, AnalyticsEventBuilder::heartbeat())
        .await;
    let second_outcome = second
        .route_analytics(&user, AnalyticsEventBuilder::heartbeat())
        .await;

    let origins: Vec<(String,)> = sqlx::query_as(
        "SELECT origin_instance_id FROM event_outbox WHERE user_id = $1 \
         ORDER BY origin_instance_id",
    )
    .bind(user.as_str())
    .fetch_all(&pool)
    .await
    .expect("listing outbox rows must succeed");
    cleanup(&pool, &user).await;

    assert!(
        matches!(first_outcome.relay, RelayOutcome::Relayed),
        "got {:?}",
        first_outcome.relay
    );
    assert!(
        matches!(second_outcome.relay, RelayOutcome::Relayed),
        "got {:?}",
        second_outcome.relay
    );
    assert_eq!(
        origins,
        vec![
            ("relay-origin-a".to_owned(),),
            ("relay-origin-b".to_owned(),)
        ],
        "each router appends exactly one row stamped with its own instance id"
    );
}

#[tokio::test]
async fn outbox_insert_failure_does_not_block_local_delivery() {
    let db = closed_db_pool().await;
    let closed = (*db.pool()).clone();
    let router = EventRouter::with_outbox(closed, InstanceId::new("origin"));

    let user = unique_user_id("relay-insert-fail");
    let conn = ConnectionId::new("relay-insert-fail-conn");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let outcome = router
        .route_analytics(&user, AnalyticsEventBuilder::heartbeat())
        .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;

    assert!(
        matches!(
            outcome.relay,
            RelayOutcome::Failed(RelayError::Persist { .. })
        ),
        "a closed outbox pool must surface as a persist failure, got {:?}",
        outcome.relay
    );
    assert_eq!(
        outcome.local, 1,
        "a failed outbox insert must not prevent local broadcast delivery"
    );
    assert!(
        rx.try_recv().is_ok(),
        "the local subscriber must still receive the event when the outbox pool is down"
    );
}
