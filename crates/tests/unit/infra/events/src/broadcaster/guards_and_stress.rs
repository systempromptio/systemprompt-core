//! Tests for connection guards, failed senders, and stress tests

use std::sync::LazyLock;
use std::time::Duration;
use systemprompt_events::{
    Broadcaster, ConnectionGuard, GenericBroadcaster,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::SystemEvent;

type TestBroadcaster = GenericBroadcaster<SystemEvent>;

fn test_user_id() -> UserId {
    UserId::new("test-user-123")
}

fn test_event() -> SystemEvent {
    systemprompt_models::SystemEventBuilder::heartbeat()
}

#[tokio::test]
async fn test_connection_guard_debug() {
    use systemprompt_events::AGUI_BROADCASTER;

    static TEST_BROADCASTER: LazyLock<GenericBroadcaster<SystemEvent>> =
        LazyLock::new(GenericBroadcaster::new);

    let guard = ConnectionGuard::new(
        &TEST_BROADCASTER,
        UserId::new("test-user"),
        "test-conn".to_string(),
    );

    let debug_str = format!("{:?}", guard);
    assert!(debug_str.contains("ConnectionGuard"));
    assert!(debug_str.contains("test-user"));
    assert!(debug_str.contains("test-conn"));

    let _ = AGUI_BROADCASTER.total_connections().await;
}

#[tokio::test]
async fn test_connection_guard_drop_unregisters() {
    static DROP_TEST_BROADCASTER: LazyLock<GenericBroadcaster<SystemEvent>> =
        LazyLock::new(GenericBroadcaster::new);

    let user_id = UserId::new("drop-test-user");
    let conn_id = "drop-test-conn".to_string();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    DROP_TEST_BROADCASTER
        .register(&user_id, &conn_id, sender)
        .await;

    assert_eq!(DROP_TEST_BROADCASTER.connection_count(&user_id).await, 1);

    {
        let _guard = ConnectionGuard::new(&DROP_TEST_BROADCASTER, user_id.clone(), conn_id.clone());
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(DROP_TEST_BROADCASTER.connection_count(&user_id).await, 0);
}

#[tokio::test]
async fn test_connection_guard_multiple_guards_same_user() {
    static MULTI_GUARD_BROADCASTER: LazyLock<GenericBroadcaster<SystemEvent>> =
        LazyLock::new(GenericBroadcaster::new);

    let user_id = UserId::new("multi-guard-user");
    let (sender1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    MULTI_GUARD_BROADCASTER
        .register(&user_id, "conn-1", sender1)
        .await;
    MULTI_GUARD_BROADCASTER
        .register(&user_id, "conn-2", sender2)
        .await;

    assert_eq!(
        MULTI_GUARD_BROADCASTER.connection_count(&user_id).await,
        2
    );

    {
        let _guard1 =
            ConnectionGuard::new(&MULTI_GUARD_BROADCASTER, user_id.clone(), "conn-1".to_string());
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        MULTI_GUARD_BROADCASTER.connection_count(&user_id).await,
        1
    );

    {
        let _guard2 =
            ConnectionGuard::new(&MULTI_GUARD_BROADCASTER, user_id.clone(), "conn-2".to_string());
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        MULTI_GUARD_BROADCASTER.connection_count(&user_id).await,
        0
    );
}

#[tokio::test]
async fn test_broadcaster_broadcast_all_failed_senders() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, rx2) = tokio::sync::mpsc::unbounded_channel();
    let (sender3, rx3) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-2", sender2).await;
    broadcaster.register(&user_id, "conn-3", sender3).await;

    drop(rx1);
    drop(rx2);
    drop(rx3);

    let count = broadcaster.broadcast(&user_id, test_event()).await;
    assert_eq!(count, 0);
    assert_eq!(broadcaster.connection_count(&user_id).await, 0);
    let connected_users = broadcaster.connected_users().await;
    assert!(connected_users.is_empty());
}

#[tokio::test]
async fn test_broadcaster_many_connections_stress() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let mut receivers = Vec::new();

    for i in 0..10 {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        broadcaster
            .register(&user_id, &format!("conn-{}", i), sender)
            .await;
        receivers.push(receiver);
    }

    assert_eq!(broadcaster.connection_count(&user_id).await, 10);
    assert_eq!(broadcaster.total_connections().await, 10);

    let count = broadcaster.broadcast(&user_id, test_event()).await;
    assert_eq!(count, 10);

    for mut rx in receivers {
        let _event = rx.recv().await.expect("Should receive broadcast on connection");
    }
}

#[tokio::test]
async fn test_broadcaster_many_users() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let mut receivers = Vec::new();

    for i in 0..5 {
        let user_id = UserId::new(&format!("user-{}", i));
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        broadcaster
            .register(&user_id, &format!("conn-{}", i), sender)
            .await;
        receivers.push((user_id, receiver));
    }

    let (users, conns) = broadcaster.connection_info().await;
    assert_eq!(users, 5);
    assert_eq!(conns, 5);

    let connected = broadcaster.connected_users().await;
    assert_eq!(connected.len(), 5);
}
