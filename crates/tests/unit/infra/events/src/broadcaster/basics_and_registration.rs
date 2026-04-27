//! Tests for broadcaster basics, constants, registration, and unregistration

use std::time::Duration;
use systemprompt_events::{
    Broadcaster, GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON, standard_keep_alive,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::SystemEvent;

type TestBroadcaster = GenericBroadcaster<SystemEvent>;

fn test_user_id() -> UserId {
    UserId::new("test-user-123")
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

#[test]
fn test_standard_keep_alive_creation() {
    let keep_alive = standard_keep_alive();
    let debug_str = format!("{:?}", keep_alive);
    assert!(debug_str.contains("KeepAlive"));
}
