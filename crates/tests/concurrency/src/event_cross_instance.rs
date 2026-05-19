//! Local-only fan-out contract for the in-process event bus.
//!
//! [`GenericBroadcaster`] fans an event out to the SSE connections held by
//! a single instance. These tests pin that boundary: an event broadcast on
//! one broadcaster instance is invisible to a subscriber registered on a
//! different instance. In a multi-replica deployment each replica owns its
//! own broadcaster, so the in-process bus alone cannot cross replicas —
//! which is exactly why the durable `event_outbox` + `PostgresEventBridge`
//! relay exists. The DB-backed counterpart in the integration suite asserts
//! that the relay closes this gap.

use std::sync::Arc;
use systemprompt_events::{Broadcaster, GenericBroadcaster};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_models::A2AEvent;
use tokio::sync::mpsc;

fn test_event() -> A2AEvent {
    use systemprompt_identifiers::{ContextId, TaskId};
    use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;

    A2AEvent::TaskStatusUpdate {
        timestamp: chrono::Utc::now(),
        payload: TaskStatusUpdatePayload {
            task_id: TaskId::generate(),
            context_id: ContextId::generate(),
            state: systemprompt_models::a2a::TaskState::Working,
            message: None,
        },
    }
}

#[tokio::test]
async fn event_does_not_cross_independent_broadcaster_instances() {
    let replica_a: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let replica_b: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());

    let user = UserId::new("user-cross-instance");
    let conn = ConnectionId::new("conn-b");

    let (tx, mut rx) = mpsc::channel(systemprompt_events::SSE_BUFFER);
    replica_b.register(&user, &conn, tx).await;

    let delivered = replica_a.broadcast(&user, test_event()).await;

    assert_eq!(
        delivered, 0,
        "replica A has no local connections; broadcast must reach nobody"
    );
    assert_eq!(
        replica_b.connection_count(&user).await,
        1,
        "the subscriber on replica B stays registered"
    );
    assert!(
        rx.try_recv().is_err(),
        "an event broadcast on replica A must NOT reach a subscriber on replica B; the in-process \
         bus cannot cross replicas without the Postgres relay"
    );
}

#[tokio::test]
async fn event_reaches_only_subscribers_on_the_same_instance() {
    let replica_a: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let replica_b: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());

    let user = UserId::new("user-same-instance");

    let (tx_a, mut rx_a) = mpsc::channel(systemprompt_events::SSE_BUFFER);
    replica_a
        .register(&user, &ConnectionId::new("conn-a"), tx_a)
        .await;

    let (tx_b, mut rx_b) = mpsc::channel(systemprompt_events::SSE_BUFFER);
    replica_b
        .register(&user, &ConnectionId::new("conn-b"), tx_b)
        .await;

    let delivered = replica_a.broadcast(&user, test_event()).await;

    assert_eq!(delivered, 1, "the broadcast reaches replica A's subscriber");
    assert!(
        rx_a.try_recv().is_ok(),
        "the same-instance subscriber receives the event"
    );
    assert!(
        rx_b.try_recv().is_err(),
        "the other-instance subscriber receives nothing"
    );
}
