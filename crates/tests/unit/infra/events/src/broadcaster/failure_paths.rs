//! Broadcast failure paths: unserializable events and dropped receivers.

use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures_util::stream;
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
        broadcaster.connected_users().is_empty(),
        "the user entry must be removed once its last connection is evicted"
    );
}
#[derive(Clone)]
struct NamedEvent(&'static str);

impl ToSse for NamedEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        Ok(Event::default().event("coverage-test").data(self.0))
    }
}

async fn event_wire(event: Event) -> String {
    let response = Sse::new(stream::once(async {
        Ok::<_, std::convert::Infallible>(event)
    }))
    .into_response();
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("SSE body");
    String::from_utf8(body.to_vec()).expect("SSE is UTF-8")
}

#[tokio::test]
async fn a_full_consumer_is_evicted_and_can_reconnect_cleanly() {
    use std::time::Duration;

    let broadcaster: GenericBroadcaster<NamedEvent> = GenericBroadcaster::new();
    let user = unique_user_id("sse-full-reconnect");
    let old_conn = ConnectionId::new("full-old");
    let (old_tx, mut old_rx) = tokio::sync::mpsc::channel(1);
    assert!(broadcaster.register(&user, &old_conn, old_tx).await);
    assert_eq!(
        broadcaster
            .broadcast(&user, NamedEvent("buffered-event"))
            .await,
        1
    );
    assert_eq!(
        broadcaster
            .broadcast(&user, NamedEvent("evicted-event"))
            .await,
        0,
        "the full consumer must not count as a delivery"
    );
    assert_eq!(broadcaster.connection_count(&user).await, 0);
    let buffered = tokio::time::timeout(Duration::from_millis(250), old_rx.recv())
        .await
        .expect("buffered receive must not stall")
        .expect("the already-buffered event remains readable")
        .expect("SSE events are infallible");
    assert_eq!(
        event_wire(buffered).await,
        "event: coverage-test\ndata: buffered-event\n\n"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(250), old_rx.recv())
            .await
            .expect("closed receive must not stall")
            .is_none(),
        "eviction closes the old stream"
    );

    let new_conn = ConnectionId::new("full-new");
    let (new_tx, mut new_rx) = tokio::sync::mpsc::channel(1);
    assert!(broadcaster.register(&user, &new_conn, new_tx).await);
    assert_eq!(
        broadcaster
            .broadcast(&user, NamedEvent("reconnected-event"))
            .await,
        1
    );
    let delivered = tokio::time::timeout(Duration::from_millis(250), new_rx.recv())
        .await
        .expect("reconnected receive must not stall")
        .expect("the new stream receives its event")
        .expect("SSE events are infallible");
    assert_eq!(
        event_wire(delivered).await,
        "event: coverage-test\ndata: reconnected-event\n\n"
    );
    assert_eq!(broadcaster.connection_count(&user).await, 1);
}
