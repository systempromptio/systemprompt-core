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
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_events::{
    A2A_BROADCASTER, Broadcaster, EventBridgeHandle, EventRouter, EventSender, PostgresEventBridge,
    RelayOutcome,
};
use systemprompt_identifiers::{ConnectionId, ContextId, InstanceId, TaskId, UserId};
use systemprompt_models::A2AEvent;
use systemprompt_models::a2a::TaskState;
use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::error::TryRecvError;

use crate::{setup_test_pool, unique_user_id};

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
    let router = EventRouter::with_outbox(pool.clone(), InstanceId::new("replica-a"));
    let bridge = PostgresEventBridge::new(pool.clone(), InstanceId::new("replica-b")).start();
    assert!(
        bounded("the replica-b relay LISTEN", bridge.listening()).await,
        "the replica-b relay stopped before it was listening"
    );
    (router, bridge)
}

async fn subscribe(user: &UserId, connection: &str) -> (ConnectionId, Receiver<Delivery>) {
    let connection = ConnectionId::new(connection);
    let (tx, rx): (EventSender, _) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    assert!(A2A_BROADCASTER.register(user, &connection, tx).await);
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
    let (connection, mut rx) = subscribe(&user, "replica-b-conn").await;

    route_on_replica_a(&router, &user).await;
    next_delivery(&mut rx, "the in-process delivery").await;
    next_delivery(&mut rx, "the relayed delivery from replica A").await;

    A2A_BROADCASTER.unregister(&user, &connection).await;
    teardown(&pool, bridge, &[&user]).await;
}

#[tokio::test]
async fn relayed_event_reaches_only_the_addressed_user() {
    let pool = setup_test_pool().await;
    let (router, bridge) = start_peer_replica(&pool).await;
    let target = unique_user_id("evt-relay");
    let bystander = unique_user_id("evt-relay");
    let (target_conn, mut target_rx) = subscribe(&target, "target-conn").await;
    let (bystander_conn, mut bystander_rx) = subscribe(&bystander, "bystander-conn").await;

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

    A2A_BROADCASTER.unregister(&target, &target_conn).await;
    A2A_BROADCASTER
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
