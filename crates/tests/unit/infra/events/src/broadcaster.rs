use std::time::Duration;
use systemprompt_events::{
    A2ABroadcaster, AgUiBroadcaster, Broadcaster, ConnectionGuard, ContextBroadcaster,
    GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON,
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

#[test]
fn test_heartbeat_json_format() {
    assert_eq!(HEARTBEAT_JSON, r#"{"type":"heartbeat"}"#);
}

#[test]
fn test_heartbeat_json_is_valid_json() {
    let result: Result<serde_json::Value, _> = serde_json::from_str(HEARTBEAT_JSON);
    assert!(result.is_ok(), "HEARTBEAT_JSON should be valid JSON");
    let parsed = result.expect("valid JSON");
    assert_eq!(parsed["type"], "heartbeat");
}

#[test]
fn test_heartbeat_interval_value() {
    assert_eq!(HEARTBEAT_INTERVAL, Duration::from_secs(15));
}

#[test]
fn test_heartbeat_interval_is_reasonable() {
    assert!(HEARTBEAT_INTERVAL >= Duration::from_secs(5));
    assert!(HEARTBEAT_INTERVAL <= Duration::from_secs(60));
}

#[test]
fn test_broadcaster_new() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let debug_str = format!("{:?}", broadcaster);
    assert!(debug_str.contains("GenericBroadcaster"));
}

#[test]
fn test_broadcaster_default() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::default();
    let debug_str = format!("{:?}", broadcaster);
    assert!(debug_str.contains("GenericBroadcaster"));
}

#[test]
fn test_broadcaster_debug_output() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let debug_str = format!("{:?}", broadcaster);
    assert!(debug_str.contains("connections"));
    assert!(debug_str.contains("RwLock"));
}

#[tokio::test]
async fn test_broadcaster_register_single_connection() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;

    assert_eq!(broadcaster.connection_count(&user_id).await, 1);
    assert_eq!(broadcaster.total_connections().await, 1);
}

#[tokio::test]
async fn test_broadcaster_register_multiple_connections_same_user() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();
    let (sender3, _rx3) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-2", sender2).await;
    broadcaster.register(&user_id, "conn-3", sender3).await;

    assert_eq!(broadcaster.connection_count(&user_id).await, 3);
    assert_eq!(broadcaster.total_connections().await, 3);
}

#[tokio::test]
async fn test_broadcaster_register_multiple_users() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user1 = UserId::new("user-1");
    let user2 = UserId::new("user-2");
    let (sender1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user1, "conn-1", sender1).await;
    broadcaster.register(&user2, "conn-2", sender2).await;

    assert_eq!(broadcaster.connection_count(&user1).await, 1);
    assert_eq!(broadcaster.connection_count(&user2).await, 1);
    assert_eq!(broadcaster.total_connections().await, 2);
}

#[tokio::test]
async fn test_broadcaster_register_replaces_same_connection_id() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-1", sender2).await;

    assert_eq!(broadcaster.connection_count(&user_id).await, 1);
}

#[tokio::test]
async fn test_broadcaster_unregister_connection() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;
    assert_eq!(broadcaster.connection_count(&user_id).await, 1);

    broadcaster.unregister(&user_id, "conn-1").await;
    assert_eq!(broadcaster.connection_count(&user_id).await, 0);
}

#[tokio::test]
async fn test_broadcaster_unregister_removes_user_when_empty() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;
    broadcaster.unregister(&user_id, "conn-1").await;

    let connected_users = broadcaster.connected_users().await;
    assert!(connected_users.is_empty());
}

#[tokio::test]
async fn test_broadcaster_unregister_nonexistent_connection() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();

    broadcaster.unregister(&user_id, "non-existent").await;
    assert_eq!(broadcaster.connection_count(&user_id).await, 0);
}

#[tokio::test]
async fn test_broadcaster_unregister_keeps_other_connections() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender1, _rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, _rx2) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender1).await;
    broadcaster.register(&user_id, "conn-2", sender2).await;
    broadcaster.unregister(&user_id, "conn-1").await;

    assert_eq!(broadcaster.connection_count(&user_id).await, 1);
}

#[tokio::test]
async fn test_broadcaster_broadcast_to_single_connection() {
    let broadcaster: TestBroadcaster = GenericBroadcaster::new();
    let user_id = test_user_id();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    broadcaster.register(&user_id, "conn-1", sender).await;
    let count = broadcaster.broadcast(&user_id, test_event()).await;

    assert_eq!(count, 1);
    assert!(receiver.recv().await.is_some());
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
    assert!(rx1.recv().await.is_some());
    assert!(rx2.recv().await.is_some());
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
    assert!(rx1.recv().await.is_some());
    assert!(rx2.try_recv().is_err());
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

#[test]
fn test_agui_broadcaster_type() {
    let _broadcaster: AgUiBroadcaster = GenericBroadcaster::new();
}

#[test]
fn test_a2a_broadcaster_type() {
    let _broadcaster: A2ABroadcaster = GenericBroadcaster::new();
}

#[test]
fn test_context_broadcaster_type() {
    let _broadcaster: ContextBroadcaster = GenericBroadcaster::new();
}

#[tokio::test]
async fn test_connection_guard_debug() {
    use std::sync::LazyLock;
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
