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
    EventRouter, OUTBOX_CHANNEL, PostgresEventBridge,
};
use systemprompt_identifiers::{ConnectionId, ContextId, EventOutboxId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    SystemEvent, SystemEventBuilder,
};
use systemprompt_test_fixtures::{
    closed_db_pool, fixture_database_url, fixture_db_pool, unique_user_id,
};

// The bridge tests each `start()` a [`PostgresEventBridge`], whose
// `PgListener` holds a long-lived connection from the shared fixture pool.
// Running several concurrently exhausts that pool and times out, so they take
// turns through this process-global async lock.
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

// Spin the relay until the subscriber receives an event or the budget is
// exhausted. Re-routing on each attempt covers the race where the bridge's
// `LISTEN` is not yet established when the first `NOTIFY` fires.
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

// `route_*` must persist a durable, queryable `event_outbox` row once the
// relay pool is installed — the handoff peer replicas read back. Installing
// the relay directly (no live bridge) keeps the shared fixture pool free of
// the listener's long-lived connection while the assertions run.
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

async fn insert_raw_outbox(
    pool: &sqlx::PgPool,
    id: &str,
    channel: &str,
    user: &UserId,
    payload: &str,
) {
    sqlx::query(
        "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id) \
         VALUES ($1, $2, $3, $4::jsonb, 'user', $5) ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(channel)
    .bind(user.as_str())
    .bind(payload)
    .bind(user.as_str())
    .execute(pool)
    .await
    .expect("insert raw outbox row must succeed");
}

async fn notify_outbox(pool: &sqlx::PgPool, id: &str) {
    let _ = sqlx::query("SELECT pg_notify($1, $2)")
        .bind(OUTBOX_CHANNEL)
        .bind(id)
        .execute(pool)
        .await;
}

// Repeatedly fire a "poison" notification (an outbox id the bridge cannot
// deliver) alongside a valid analytics route, asserting the bridge survives
// the poison branch and still delivers the good event. The valid delivery is
// the deterministic signal; the poison exercises `deliver`/`fan_in` error
// arms.
async fn relay_survives_poison<Fp>(
    poison: Fp,
    user: &UserId,
    rx: &mut tokio::sync::mpsc::Receiver<R>,
) -> bool
where
    Fp: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
{
    for _ in 0..20 {
        poison().await;
        EventRouter::route_analytics(user, analytics_event()).await;
        if let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn bridge_survives_missing_outbox_row_notification() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-missing-row");
    let conn = ConnectionId::new("bridge-missing-row-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let poison_pool = pool.clone();
    let delivered = relay_survives_poison(
        move || {
            let pool = poison_pool.clone();
            Box::pin(async move {
                notify_outbox(&pool, &EventOutboxId::generate().as_str().to_owned()).await;
            })
        },
        &user,
        &mut rx,
    )
    .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "a NOTIFY for a pruned/absent outbox id must be skipped without stalling the relay"
    );
}

#[tokio::test]
async fn bridge_survives_unknown_channel_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-bad-channel");
    let conn = ConnectionId::new("bridge-bad-channel-conn");

    let bad_id = EventOutboxId::generate().as_str().to_owned();
    insert_raw_outbox(&pool, &bad_id, "not-a-real-channel", &user, "{}").await;

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let poison_pool = pool.clone();
    let poison_id = bad_id.clone();
    let delivered = relay_survives_poison(
        move || {
            let pool = poison_pool.clone();
            let id = poison_id.clone();
            Box::pin(async move {
                notify_outbox(&pool, &id).await;
            })
        },
        &user,
        &mut rx,
    )
    .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "an outbox row with an unparseable channel must be logged and skipped, not stall the relay"
    );
}

#[tokio::test]
async fn bridge_survives_undecodable_payload() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-bad-payload");
    let conn = ConnectionId::new("bridge-bad-payload-conn");

    let bad_id = EventOutboxId::generate().as_str().to_owned();
    insert_raw_outbox(&pool, &bad_id, "agui", &user, r#"{"unexpected":"shape"}"#).await;

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let poison_pool = pool.clone();
    let poison_id = bad_id.clone();
    let delivered = relay_survives_poison(
        move || {
            let pool = poison_pool.clone();
            let id = poison_id.clone();
            Box::pin(async move {
                notify_outbox(&pool, &id).await;
            })
        },
        &user,
        &mut rx,
    )
    .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "a row whose payload fails to decode for its channel must be logged and skipped"
    );
}

#[tokio::test]
async fn bridge_survives_undecodable_payloads_on_every_channel() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-bad-all");
    let conn = ConnectionId::new("bridge-bad-all-conn");

    let mut bad_ids = Vec::new();
    for channel in ["a2a", "system", "analytics"] {
        let id = EventOutboxId::generate().as_str().to_owned();
        insert_raw_outbox(&pool, &id, channel, &user, r#"{"unexpected":"shape"}"#).await;
        bad_ids.push(id);
    }

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let poison_pool = pool.clone();
    let poison_ids = bad_ids.clone();
    let delivered = relay_survives_poison(
        move || {
            let pool = poison_pool.clone();
            let ids = poison_ids.clone();
            Box::pin(async move {
                for id in &ids {
                    notify_outbox(&pool, id).await;
                }
            })
        },
        &user,
        &mut rx,
    )
    .await;

    ANALYTICS_BROADCASTER.unregister(&user, &conn).await;
    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        delivered,
        "undecodable payloads on the A2A, system, and analytics channels must each be logged and \
         skipped without stalling the relay"
    );
}

async fn terminate_outbox_listeners(pool: &sqlx::PgPool) {
    let _ = sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid != \
         pg_backend_pid() AND query ILIKE '%LISTEN \"event_outbox%'",
    )
    .execute(pool)
    .await;
}

#[tokio::test]
async fn bridge_reconnects_after_listener_connection_is_terminated() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-reconnect");
    let conn = ConnectionId::new("bridge-reconnect-conn");

    let handle = PostgresEventBridge::new(pool.clone()).start();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<R>(systemprompt_events::SSE_BUFFER);
    ANALYTICS_BROADCASTER.register(&user, &conn, tx).await;

    let user_for_route = user.clone();
    let delivered_before = relay_until_delivered(
        move || {
            let user = user_for_route.clone();
            Box::pin(async move {
                EventRouter::route_analytics(&user, analytics_event()).await;
            })
        },
        &mut rx,
    )
    .await;
    assert!(delivered_before, "bridge must deliver before the kill");

    terminate_outbox_listeners(&pool).await;
    while rx.try_recv().is_ok() {}

    let user_for_route = user.clone();
    let delivered_after = relay_until_delivered(
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
        delivered_after,
        "after its LISTEN connection is terminated the bridge must reconnect and resume delivery"
    );
}

// Paused time makes the 5-second retry back-off instantaneous, so several
// connect-fail/sleep iterations run without any wall-clock cost.
#[tokio::test(start_paused = true)]
async fn bridge_survives_listener_connect_failure_and_keeps_retrying() {
    let db = closed_db_pool().await;
    let pool = (*db
        .pool_arc()
        .expect("closed fixture pool must expose a pg pool"))
    .clone();

    let handle = PostgresEventBridge::new(pool).start();
    tokio::time::sleep(Duration::from_secs(30)).await;

    assert!(
        !handle.is_finished(),
        "the bridge must keep retrying when the listener cannot connect, not exit"
    );
    handle.abort();
    let err = handle
        .await
        .expect_err("an aborted bridge task must resolve to a JoinError");
    assert!(
        err.is_cancelled(),
        "the bridge task must be cancelled by abort, not have panicked"
    );
}

async fn insert_outbox_with_age(pool: &sqlx::PgPool, id: &str, user: &UserId, age: &str) {
    sqlx::query(
        "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, \
         created_at) VALUES ($1, 'system', $2, '{}'::jsonb, 'user', $2, now() - $3::interval)",
    )
    .bind(id)
    .bind(user.as_str())
    .bind(age)
    .execute(pool)
    .await
    .expect("insert aged outbox row must succeed");
}

async fn outbox_row_exists(pool: &sqlx::PgPool, id: &str) -> bool {
    let row: (i64,) = sqlx::query_as("SELECT count(*) FROM event_outbox WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("counting outbox rows must succeed");
    row.0 > 0
}

// The prune tick fires every 300 seconds, far beyond any test budget, so the
// clock is briefly paused and advanced past the interval on each attempt;
// the DELETE itself then runs in resumed real time.
#[tokio::test]
async fn bridge_prune_deletes_expired_rows_and_keeps_fresh_ones() {
    let Some(pool) = pool().await else {
        return;
    };
    let _guard = BRIDGE_LOCK.lock().await;
    let user = unique_user_id("bridge-prune");
    let old_id = EventOutboxId::generate().as_str().to_owned();
    let fresh_id = EventOutboxId::generate().as_str().to_owned();
    insert_outbox_with_age(&pool, &old_id, &user, "2 hours").await;
    insert_outbox_with_age(&pool, &fresh_id, &user, "0 seconds").await;

    let handle = PostgresEventBridge::new(pool.clone()).start();

    let mut old_pruned = false;
    for _ in 0..40 {
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(301)).await;
        tokio::time::resume();
        tokio::time::sleep(Duration::from_millis(100)).await;
        if !outbox_row_exists(&pool, &old_id).await {
            old_pruned = true;
            break;
        }
    }
    let fresh_survived = outbox_row_exists(&pool, &fresh_id).await;

    handle.abort();
    cleanup(&pool, &user).await;

    assert!(
        old_pruned,
        "a prune tick must delete outbox rows older than the retention window"
    );
    assert!(
        fresh_survived,
        "prune must keep rows younger than the retention window"
    );
}
