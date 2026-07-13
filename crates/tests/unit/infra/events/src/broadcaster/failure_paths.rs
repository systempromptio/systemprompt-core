//! Broadcast failure paths: unserializable events and dropped receivers.

use axum::response::sse::Event;
use systemprompt_events::{Broadcaster, GenericBroadcaster, ToSse};
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::SystemEvent;
use systemprompt_test_fixtures::unique_user_id;

#[derive(Clone)]
struct UnserializableEvent;

impl ToSse for UnserializableEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serde_json::from_str::<serde_json::Value>("not json").map(|_| Event::default())
    }
}

#[tokio::test]
async fn broadcast_returns_zero_when_event_fails_to_serialize() {
    let broadcaster: GenericBroadcaster<UnserializableEvent> = GenericBroadcaster::new();
    let user = unique_user_id("sse-serialize-fail");
    let conn = ConnectionId::new("serialize-fail-conn");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    broadcaster.register(&user, &conn, tx).await;

    let count = broadcaster.broadcast(&user, UnserializableEvent).await;

    assert_eq!(
        count, 0,
        "an event whose SSE serialization fails must be delivered to nobody"
    );
    assert!(
        rx.try_recv().is_err(),
        "no SSE frame may reach the subscriber when serialization fails"
    );
    assert_eq!(
        broadcaster.connection_count(&user).await,
        1,
        "a serialization failure must not evict the connection"
    );
}

#[tokio::test]
async fn broadcast_to_dropped_receiver_evicts_user_entry() {
    let broadcaster: GenericBroadcaster<SystemEvent> = GenericBroadcaster::new();
    let user = unique_user_id("sse-dropped-rx");
    let conn = ConnectionId::new("dropped-rx-conn");
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    broadcaster.register(&user, &conn, tx).await;
    drop(rx);

    let count = broadcaster
        .broadcast(&user, systemprompt_models::SystemEventBuilder::heartbeat())
        .await;

    assert_eq!(count, 0, "a closed channel must not count as delivered");
    assert_eq!(
        broadcaster.connection_count(&user).await,
        0,
        "the failed connection must be evicted"
    );
    assert!(
        broadcaster.connected_users().await.is_empty(),
        "the user entry must be removed once its last connection is evicted"
    );
}
