//! Tests for broadcasting messages and connection info queries

use systemprompt_events::{Broadcaster, GenericBroadcaster};
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
async fn test_broadcaster_broadcast_to_single_connection() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;
    let count = broadcaster.broadcast(&user_id, test_event()).await;

    assert_eq!(count, 1);
    let _event = receiver.recv().await.expect("Should receive broadcast message");
}

#[tokio::test]
async fn test_broadcaster_broadcast_to_multiple_connections() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-2", sender2).await;
    let count = broadcaster.broadcast(&user_id, test_event()).await;

    assert_eq!(count, 2);
    let _event1 = rx1.recv().await.expect("Should receive on connection 1");
    let _event2 = rx2.recv().await.expect("Should receive on connection 2");
}

#[tokio::test]
async fn test_broadcaster_broadcast_to_nonexistent_user() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();

    let count = broadcaster.broadcast(&user_id, test_event()).await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_broadcaster_broadcast_removes_failed_senders() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-2", sender2).await;

    drop(rx1);

    let count = broadcaster.broadcast(&user_id, test_event()).await;
    assert_eq!(count, 1);
    assert_eq!(broadcaster.connection_count(&user_id).await, 1);
}

#[tokio::test]
async fn test_broadcaster_broadcast_only_to_target_user() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user1 = UserId::new("user-1");
    let user2 = UserId::new("user-2");
    let (sender1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user1, "conn-1", sender1).await;
    broadcaster.register(&user2, "conn-2", sender2).await;

    let count = broadcaster.broadcast(&user1, test_event()).await;

    assert_eq!(count, 1);
    let _event = rx1.recv().await.expect("Should receive on user1 connection");
    rx2.try_recv().unwrap_err();
}

#[tokio::test]
async fn test_broadcaster_connection_count_empty() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();

    assert_eq!(broadcaster.connection_count(&user_id).await, 0);
}

#[tokio::test]
async fn test_broadcaster_total_connections_empty() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    assert_eq!(broadcaster.total_connections().await, 0);
}

#[tokio::test]
async fn test_broadcaster_total_connections_multiple_users() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user1 = UserId::new("user-1");
    let user2 = UserId::new("user-2");
    let (s1, _r1) = tokio::sync::mpsc::unbounded_channel();
    let (s2, _r2) = tokio::sync::mpsc::unbounded_channel();
    let (s3, _r3) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user1, "conn-1", s1).await;
    broadcaster.register(&user1, "conn-2", s2).await;
    broadcaster.register(&user2, "conn-3", s3).await;

    assert_eq!(broadcaster.total_connections().await, 3);
}

#[tokio::test]
async fn test_broadcaster_connected_users_empty() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let users = broadcaster.connected_users().await;
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_broadcaster_connected_users_single() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;
    let users = broadcaster.connected_users().await;

    assert_eq!(users.len(), 1);
    assert!(users.contains(&user_id.to_string()));
}

#[tokio::test]
async fn test_broadcaster_connected_users_multiple() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let first_user = UserId::new("user-1");
    let second_user = UserId::new("user-2");
    let (s1, _r1) = tokio::sync::mpsc::unbounded_channel();
    let (s2, _r2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&first_user, "conn-1", s1).await;
    broadcaster.register(&second_user, "conn-2", s2).await;
    let connected = broadcaster.connected_users().await;

    assert_eq!(connected.len(), 2);
    assert!(connected.contains(&first_user.to_string()));
    assert!(connected.contains(&second_user.to_string()));
}

#[tokio::test]
async fn test_broadcaster_connection_info_empty() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let (user_count, conn_count) = broadcaster.connection_info().await;
    assert_eq!(user_count, 0);
    assert_eq!(conn_count, 0);
}

#[tokio::test]
async fn test_broadcaster_connection_info_with_data() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user1 = UserId::new("user-1");
    let user2 = UserId::new("user-2");
    let (s1, _r1) = tokio::sync::mpsc::unbounded_channel();
    let (s2, _r2) = tokio::sync::mpsc::unbounded_channel();
    let (s3, _r3) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user1, "conn-1", s1).await;
    broadcaster.register(&user1, "conn-2", s2).await;
    broadcaster.register(&user2, "conn-3", s3).await;

    let (user_count, conn_count) = broadcaster.connection_info().await;
    assert_eq!(user_count, 2);
    assert_eq!(conn_count, 3);
}
