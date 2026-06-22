//! DB-backed tests for the cross-replica event relay.
//!
//! These drive [`PostgresEventBridge`] end-to-end against a real, migrated
//! Postgres `event_outbox`: an event routed via [`EventRouter::route_*`] is
//! persisted + `NOTIFY`d, the running bridge consumes the notification,
//! reloads the row, decodes it by channel, and re-injects it through the
//! local broadcaster a subscriber is attached to. This exercises the bridge's
//! `deliver`/`fan_in` arms and the outbox repository's `insert`/`notify`/
//! `find` queries that no in-process broadcaster test reaches.

use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, Broadcaster, CONTEXT_BROADCASTER,
    EventRouter, PostgresEventBridge,
};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    SystemEvent, SystemEventBuilder,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, unique_user_id};

/// The bridge tests each `start()` a [`PostgresEventBridge`], whose
/// `PgListener` holds a long-lived connection from the shared fixture pool.
/// Running several concurrently exhausts that pool and times out, so they take
/// turns through this process-global async lock.
static BRIDGE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

fn agui_event() -> AgUiEvent {
    AgUiEventBuilder::run_started(ContextId::generate(), TaskId::generate(), None)
}

fn a2a_event() -> A2AEvent {
    A2AEventBuilder::task_status_update(
        TaskId::generate(),
        ContextId::generate(),
        TaskState::Working,
        None,
    )
}

fn system_event() -> SystemEvent {
    SystemEventBuilder::heartbeat()
}

fn analytics_event() -> AnalyticsEvent {
    AnalyticsEventBuilder::heartbeat()
}

/// Spin the relay until the subscriber receives an event or the budget is
/// exhausted. Re-routing on each attempt covers the race where the bridge's
/// `LISTEN` is not yet established when the first `NOTIFY` fires.
async fn relay_until_delivered<F>(route: F, rx: &mut tokio::sync::mpsc::Receiver<R>) -> bool
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
{
    for _ in 0..20 {
        route().await;
        if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            return true;
        }
    }
    false
}

type R = Result<axum::response::sse::Event, std::convert::Infallible>;

#[tokio::test]
async fn agui_event_relays_through_bridge_to_local_subscriber() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-agui");
    let conn = ConnectionId::new("bridge-agui-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    AGUI_BROADCASTER.register(&user, &conn, tx).await;

    let user_for_route = user.clone();
    let delivered = relay_until_delivered(
        move || {
            let user = user_for_route.clone();
            Box::pin(async move {
                EventRouter::route_agui(&user, agui_event()).await;
            })
        },
        &mut rx,
    )
    .await;

    AGUI_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "AG-UI event routed on one replica must reach the local subscriber via the bridge"
    );
}

#[tokio::test]
async fn a2a_event_relays_through_bridge_to_local_subscriber() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-a2a");
    let conn = ConnectionId::new("bridge-a2a-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    A2A_BROADCASTER.register(&user, &conn, tx).await;

    let user_for_route = user.clone();
    let delivered = relay_until_delivered(
        move || {
            let user = user_for_route.clone();
            Box::pin(async move {
                EventRouter::route_a2a(&user, a2a_event()).await;
            })
        },
        &mut rx,
    )
    .await;

    A2A_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "A2A event routed on one replica must reach the local subscriber via the bridge"
    );
}

#[tokio::test]
async fn system_event_relays_through_bridge_to_context_subscriber() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-system");
    let conn = ConnectionId::new("bridge-system-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    CONTEXT_BROADCASTER.register(&user, &conn, tx).await;

    let user_for_route = user.clone();
    let delivered = relay_until_delivered(
        move || {
            let user = user_for_route.clone();
            Box::pin(async move {
                EventRouter::route_system(&user, system_event()).await;
            })
        },
        &mut rx,
    )
    .await;

    CONTEXT_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "system event routed on one replica must reach the context subscriber via the bridge"
    );
}

#[tokio::test]
async fn analytics_event_relays_through_bridge_to_local_subscriber() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-analytics");
    let conn = ConnectionId::new("bridge-analytics-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let user_for_route = user.clone();
    let delivered = relay_until_delivered(
        move || {
            let user = user_for_route.clone();
            Box::pin(async move {
                EventRouter::route_analytics(&user, analytics_event()).await;
            })
        },
        &mut rx,
    )
    .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "analytics event routed on one replica must reach the local subscriber via the bridge"
    );
}

/// `route_*` must persist a durable, queryable `event_outbox` row once the
/// relay pool is installed — the handoff peer replicas read back. Installing
/// the relay directly (no live bridge) keeps the shared fixture pool free of
/// the listener's long-lived connection while the assertions run.
#[tokio::test]
async fn route_persists_queryable_outbox_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-persist");

    EventRouter::install_relay(pool.clone());
    EventRouter::route_analytics(&user, analytics_event()).await;
    EventRouter::route_system(&user, system_event()).await;

    let channels: Vec<(String,)> =
        sqlx::query_as("SELECT channel FROM event_outbox WHERE user_id = $1 ORDER BY channel")
            .bind(user.as_str())
            .fetch_all(&pool)
            .await
            .expect("reading persisted channels must succeed");

    cleanup(&pool, &user).await;

    let channel_names: Vec<&str> = channels.iter().map(|c| c.0.as_str()).collect();
    assert!(
        channel_names.contains(&"analytics") && channel_names.contains(&"system"),
        "each route_* call must append a durable outbox row recording its channel, got \
         {channel_names:?}"
    );
}
