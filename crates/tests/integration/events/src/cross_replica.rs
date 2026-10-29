//! Cross-replica event relay.
//!
//! Invariant under test: an event routed on one replica reaches an SSE
//! subscriber attached to a *different* replica, and only the addressed user.
//! [`EventRouter::route_a2a`] broadcasts in-process and appends an
//! `event_outbox` row announced by `NOTIFY`; the [`PostgresEventBridge`] of
//! every other replica loads that row and re-injects the event into its own
//! broadcasters. A bridge skips rows its own instance wrote, because the
//! local broadcast already delivered them.
//!
//! One process plays both replicas. The router relays as `replica-a` and the
//! bridge starts as `replica-b`, so every routed row is
//! foreign to the bridge and is relayed. A subscriber therefore receives each
//! event twice: once from the in-process broadcast and once through the
//! relay. Only the second delivery proves the relay; without it the test
//! would pass on the local broadcast alone.
//!
//! Every wait on the database or on a delivery is bounded, so a regression
//! fails within seconds.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use systemprompt_events::{
    Broadcaster, EventBridgeHandle, EventBroadcasters, EventRouter, EventSender,
    PostgresEventBridge, RelayOutcome,
};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId, UserId};
use systemprompt_models::A2AEvent;
use systemprompt_models::a2a::TaskState;
use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::error::TryRecvError;

use crate::{setup_test_pool, unique_user_id};
use systemprompt_test_fixtures::unique_instance;

const BOUND: Duration = Duration::from_secs(10);

type Delivery = Result<axum::response::sse::Event, std::convert::Infallible>;

async fn bounded<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(BOUND, future)
        .await
        .unwrap_or_else(|_| panic!("{what} did not complete within {BOUND:?}"))
}

fn sample_event() -> A2AEvent {
    A2AEvent::TaskStatusUpdate {
        timestamp: chrono::Utc::now(),
        payload: TaskStatusUpdatePayload {
            task_id: TaskId::generate(),
            context_id: ContextId::generate(),
            state: TaskState::Working,
            message: None,
        },
    }
}

async fn start_peer_replica(pool: &PgPool) -> (EventRouter, EventBridgeHandle) {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let router =
        EventRouter::with_outbox(pool.clone(), unique_instance(), Arc::clone(&broadcasters));
    let bridge = PostgresEventBridge::new(
        pool.clone(),
        unique_instance(),
        systemprompt_traits::CacheEpochs::new(),
        broadcasters,
    )
    .start();
    assert!(
        bounded("the replica-b relay LISTEN", bridge.listening()).await,
        "the replica-b relay stopped before it was listening"
    );
    (router, bridge)
}

async fn subscribe(
    router: &EventRouter,
    user: &UserId,
    connection: &str,
) -> (ConnectionId, Receiver<Delivery>) {
    let connection = ConnectionId::new(connection);
    let (tx, rx): (EventSender, _) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    assert!(
        router
            .broadcasters()
            .a2a
            .register(user, &connection, tx)
            .await
    );
    (connection, rx)
}

async fn route_on_replica_a(router: &EventRouter, user: &UserId) {
    let outcome = bounded("route_a2a", router.route_a2a(user, sample_event())).await;
    assert!(
        matches!(outcome.relay, RelayOutcome::Relayed),
        "replica A must hand the event to the outbox: {:?}",
        outcome.relay
    );
}

async fn next_delivery(rx: &mut Receiver<Delivery>, what: &str) {
    let item = bounded(what, rx.recv())
        .await
        .unwrap_or_else(|| panic!("broadcaster channel closed before {what}"));
    assert!(item.is_ok(), "{what} must encode as an SSE event");
}

async fn teardown(pool: &PgPool, bridge: EventBridgeHandle, users: &[&UserId]) {
    bounded("relay shutdown", bridge.shutdown()).await;
    let ids: Vec<String> = users.iter().map(|u| u.as_str().to_owned()).collect();
    bounded(
        "outbox cleanup",
        sqlx::query("DELETE FROM event_outbox WHERE user_id = ANY($1)")
            .bind(&ids)
            .execute(pool),
    )
    .await
    .expect("outbox cleanup");
}

#[tokio::test]
async fn event_routed_on_replica_a_reaches_subscriber_on_replica_b() {
    let pool = setup_test_pool().await;
    let (router, bridge) = start_peer_replica(&pool).await;
    let user = unique_user_id("evt-relay");
    let (connection, mut rx) = subscribe(&router, &user, "replica-b-conn").await;

    route_on_replica_a(&router, &user).await;
    next_delivery(&mut rx, "the in-process delivery").await;
    next_delivery(&mut rx, "the relayed delivery from replica A").await;

    router
        .broadcasters()
        .a2a
        .unregister(&user, &connection)
        .await;
    teardown(&pool, bridge, &[&user]).await;
}

#[tokio::test]
async fn relayed_event_reaches_only_the_addressed_user() {
    let pool = setup_test_pool().await;
    let (router, bridge) = start_peer_replica(&pool).await;
    let target = unique_user_id("evt-relay");
    let bystander = unique_user_id("evt-relay");
    let (target_conn, mut target_rx) = subscribe(&router, &target, "target-conn").await;
    let (bystander_conn, mut bystander_rx) = subscribe(&router, &bystander, "bystander-conn").await;

    route_on_replica_a(&router, &target).await;
    next_delivery(&mut target_rx, "the target's in-process delivery").await;
    next_delivery(&mut target_rx, "the target's relayed delivery").await;

    // Why: the relay re-injects notifications in commit order on one LISTEN
    // session, so a leak of the target's relayed event would sit in the
    // bystander's channel ahead of the fence's relayed copy.
    route_on_replica_a(&router, &bystander).await;
    next_delivery(&mut bystander_rx, "the fence's in-process delivery").await;
    next_delivery(&mut bystander_rx, "the fence's relayed delivery").await;

    let bystander_extra = bystander_rx.try_recv();
    let target_extra = target_rx.try_recv();

    router
        .broadcasters()
        .a2a
        .unregister(&target, &target_conn)
        .await;
    router
        .broadcasters()
        .a2a
        .unregister(&bystander, &bystander_conn)
        .await;
    teardown(&pool, bridge, &[&target, &bystander]).await;

    assert!(
        matches!(bystander_extra, Err(TryRecvError::Empty)),
        "an unrelated user must not receive another user's relayed event"
    );
    assert!(
        matches!(target_extra, Err(TryRecvError::Empty)),
        "the addressed user must not receive the bystander's event"
    );
}

async fn start_replica_with_named_listener(
    pool: &PgPool,
    application_name: &str,
) -> (EventRouter, EventBridgeHandle) {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let router =
        EventRouter::with_outbox(pool.clone(), unique_instance(), Arc::clone(&broadcasters));
    let options = (*pool.connect_options())
        .clone()
        .application_name(application_name);
    let listener_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .expect("listener pool");
    let bridge = PostgresEventBridge::new(
        pool.clone(),
        unique_instance(),
        systemprompt_traits::CacheEpochs::new(),
        broadcasters,
    )
    .with_listener_pool(listener_pool)
    .start();
    assert!(
        bounded("the relay LISTEN", bridge.listening()).await,
        "the relay stopped before it was listening"
    );
    (router, bridge)
}

async fn insert_unannounced_row(pool: &PgPool, user: &UserId) {
    let payload = serde_json::to_value(sample_event()).expect("event payload");
    bounded(
        "insert an outbox row without NOTIFY",
        sqlx::query(
            "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, \
             origin_instance_id) VALUES ($1, 'a2a', $2, $3, 'user', $2, 'replica-elsewhere')",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user.as_str())
        .bind(payload)
        .execute(pool),
    )
    .await
    .expect("insert outbox row");
}

#[tokio::test]
async fn a_row_written_while_the_relay_was_disconnected_is_replayed_on_reconnect() {
    let pool = setup_test_pool().await;
    let application_name = format!("bridge-replay-{}", uuid::Uuid::new_v4().simple());
    let (router, bridge) = start_replica_with_named_listener(&pool, &application_name).await;
    let user = unique_user_id("evt-replay");
    let (connection, mut rx) = subscribe(&router, &user, "replica-b-replay").await;

    insert_unannounced_row(&pool, &user).await;
    let terminated: Vec<bool> = bounded(
        "terminate this relay's listener",
        sqlx::query_scalar(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE application_name = $1 AND pid <> pg_backend_pid()",
        )
        .bind(&application_name)
        .fetch_all(&*pool),
    )
    .await
    .expect("terminate listener");
    assert!(
        terminated.iter().any(|done| *done),
        "the relay's own listener session must be found and terminated"
    );

    next_delivery(&mut rx, "the replayed delivery after reconnect").await;

    router
        .broadcasters()
        .a2a
        .unregister(&user, &connection)
        .await;
    teardown(&pool, bridge, &[&user]).await;
}
