//! Regression test for the cross-replica event relay (D2).
//!
//! Invariant under test: an event routed on one replica reaches an SSE
//! subscriber attached to a *different* replica. The in-process broadcasters
//! only fan out within a single process; [`EventRouter::route_a2a`] also
//! appends an `event_outbox` row and emits a Postgres `NOTIFY`, and the
//! [`PostgresEventBridge`] running on every replica consumes that
//! notification and re-injects the event into its local broadcasters.
//!
//! Before D2, `route_a2a` was local-only and this test fails: the subscriber
//! on "replica B" never sees the event. After D2 it passes.
//!
//! The test drives the relay entirely through the public surface — no
//! `pub(crate)` internals are reached. "Replica A" is the `route_a2a` call;
//! "replica B" is a subscriber on the process-global `A2A_BROADCASTER`, which
//! is the broadcaster the bridge's local re-injection path writes to.

use std::time::Duration;

use systemprompt_events::{A2A_BROADCASTER, Broadcaster, EventRouter, PostgresEventBridge};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId, UserId};
use systemprompt_models::A2AEvent;
use systemprompt_models::a2a::TaskState;
use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;

use crate::{ensure_event_outbox, setup_test_pool};
use systemprompt_test_fixtures::unique_user_id;

fn unique_user() -> UserId {
    unique_user_id("evt-relay")
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

#[tokio::test]
async fn event_routed_on_replica_a_reaches_subscriber_on_replica_b() {
    let pool = setup_test_pool().await;
    ensure_event_outbox(&pool).await;

    let user = unique_user();
    let connection = ConnectionId::new("replica-b-conn");

    let bridge = PostgresEventBridge::new((*pool).clone());
    let bridge_handle = bridge.start();

    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    A2A_BROADCASTER.register(&user, &connection, tx).await;

    let received = {
        let mut delivered = None;
        for _ in 0..20 {
            EventRouter::route_a2a(&user, sample_event()).await;

            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(item)) => {
                    delivered = Some(item);
                    break;
                },
                Ok(None) => panic!("broadcaster channel closed before delivery"),
                Err(_) => {},
            }
        }
        delivered
    };

    A2A_BROADCASTER.unregister(&user, &connection).await;
    bridge_handle.abort();
    let _ = sqlx::query("DELETE FROM event_outbox WHERE user_id = $1")
        .bind(user.as_str())
        .execute(pool.as_ref())
        .await;

    let item = received.expect(
        "event routed on replica A never reached the subscriber on replica B — the cross-replica \
         outbox relay is not delivering events",
    );
    assert!(
        item.is_ok(),
        "the relayed SSE event must deserialize cleanly"
    );
}

#[tokio::test]
async fn relayed_event_reaches_only_the_addressed_user() {
    let pool = setup_test_pool().await;
    ensure_event_outbox(&pool).await;

    let target = unique_user();
    let bystander = unique_user();

    let bridge = PostgresEventBridge::new((*pool).clone());
    let bridge_handle = bridge.start();

    let (target_tx, mut target_rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let (bystander_tx, mut bystander_rx) =
        tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let target_conn = ConnectionId::new("target-conn");
    let bystander_conn = ConnectionId::new("bystander-conn");
    A2A_BROADCASTER
        .register(&target, &target_conn, target_tx)
        .await;
    A2A_BROADCASTER
        .register(&bystander, &bystander_conn, bystander_tx)
        .await;

    let mut delivered = false;
    for _ in 0..20 {
        EventRouter::route_a2a(&target, sample_event()).await;
        match tokio::time::timeout(Duration::from_millis(500), target_rx.recv()).await {
            Ok(Some(_)) => {
                delivered = true;
                break;
            },
            Ok(None) => panic!("broadcaster channel closed before delivery"),
            Err(_) => {},
        }
    }

    let bystander_saw = bystander_rx.try_recv().is_ok();

    A2A_BROADCASTER.unregister(&target, &target_conn).await;
    A2A_BROADCASTER
        .unregister(&bystander, &bystander_conn)
        .await;
    bridge_handle.abort();
    let _ = sqlx::query("DELETE FROM event_outbox WHERE user_id = $1")
        .bind(target.as_str())
        .execute(pool.as_ref())
        .await;

    assert!(
        delivered,
        "the addressed user must receive the relayed event"
    );
    assert!(
        !bystander_saw,
        "an unrelated user must not receive another user's relayed event"
    );
}
