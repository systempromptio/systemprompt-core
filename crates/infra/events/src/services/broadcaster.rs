use async_trait::async_trait;
use axum::response::sse::{Event, KeepAlive};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::UserId;
use tokio::sync::RwLock;

use crate::{Broadcaster, EventSender};
use systemprompt_models::events::ToSse;

pub const HEARTBEAT_JSON: &str = r#"{"type":"heartbeat"}"#;
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

pub fn standard_keep_alive() -> KeepAlive {
    KeepAlive::new()
        .interval(HEARTBEAT_INTERVAL)
        .event(Event::default().event("heartbeat").data(HEARTBEAT_JSON))
}

pub struct GenericBroadcaster<E: ToSse + Clone + Send + Sync> {
    connections: Arc<RwLock<HashMap<String, HashMap<String, EventSender>>>>,
    _phantom: PhantomData<E>,
}

impl<E: ToSse + Clone + Send + Sync> std::fmt::Debug for GenericBroadcaster<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericBroadcaster")
            .field("connections", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl<E: ToSse + Clone + Send + Sync> GenericBroadcaster<E> {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            _phantom: PhantomData,
        }
    }

    pub async fn connected_users(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    pub async fn connection_info(&self) -> (usize, usize) {
        let (user_count, conn_count) = {
            let connections = self.connections.read().await;
            (
                connections.len(),
                connections.values().map(HashMap::len).sum(),
            )
        };
        (user_count, conn_count)
    }
}

impl<E: ToSse + Clone + Send + Sync> Default for GenericBroadcaster<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<E: ToSse + Clone + Send + Sync + 'static> Broadcaster for GenericBroadcaster<E> {
    type Event = E;

    #[allow(clippy::significant_drop_tightening)] // Lock must be held for entry().or_default().insert()
    async fn register(&self, user_id: &UserId, connection_id: &str, sender: EventSender) {
        let mut connections = self.connections.write().await;
        let user_connections = connections.entry(user_id.to_string()).or_default();
        user_connections.insert(connection_id.to_string(), sender);
    }

    async fn unregister(&self, user_id: &UserId, connection_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(user_connections) = connections.get_mut(user_id.as_str()) {
            user_connections.remove(connection_id);
            if user_connections.is_empty() {
                connections.remove(user_id.as_str());
            }
        }
    }

    async fn broadcast(&self, user_id: &UserId, event: Self::Event) -> usize {
        let sse_event: Event = match event.to_sse() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, event_type = ?std::any::type_name_of_val(&event), "Failed to serialize SSE event");
                return 0;
            },
        };

        let senders: Vec<(String, EventSender)> = {
            let connections = self.connections.read().await;
            match connections.get(user_id.as_str()) {
                Some(user_connections) => user_connections
                    .iter()
                    .map(|(id, sender)| (id.clone(), sender.clone()))
                    .collect(),
                None => return 0,
            }
        };

        let mut successful = 0;
        let mut failed_ids = Vec::new();

        for (conn_id, sender) in senders {
            if sender.send(Ok(sse_event.clone())).is_ok() {
                successful += 1;
            } else {
                failed_ids.push(conn_id);
            }
        }

        if !failed_ids.is_empty() {
            let mut connections = self.connections.write().await;
            if let Some(user_connections) = connections.get_mut(user_id.as_str()) {
                for conn_id in &failed_ids {
                    user_connections.remove(conn_id);
                }
                if user_connections.is_empty() {
                    connections.remove(user_id.as_str());
                }
            }
        }

        successful
    }

    async fn connection_count(&self, user_id: &UserId) -> usize {
        let connections = self.connections.read().await;
        connections.get(user_id.as_str()).map_or(0, HashMap::len)
    }

    async fn total_connections(&self) -> usize {
        let connections = self.connections.read().await;
        connections.values().map(HashMap::len).sum()
    }
}

use systemprompt_models::{A2AEvent, AgUiEvent, ContextEvent};

pub type AgUiBroadcaster = GenericBroadcaster<AgUiEvent>;
pub type A2ABroadcaster = GenericBroadcaster<A2AEvent>;
pub type ContextBroadcaster = GenericBroadcaster<ContextEvent>;

pub struct ConnectionGuard<E: ToSse + Clone + Send + Sync + 'static> {
    broadcaster: &'static std::sync::LazyLock<GenericBroadcaster<E>>,
    user_id: UserId,
    connection_id: String,
}

#[allow(clippy::missing_fields_in_debug)]
impl<E: ToSse + Clone + Send + Sync + 'static> std::fmt::Debug for ConnectionGuard<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionGuard")
            .field("user_id", &self.user_id)
            .field("connection_id", &self.connection_id)
            .finish_non_exhaustive()
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> ConnectionGuard<E> {
    pub fn new(
        broadcaster: &'static std::sync::LazyLock<GenericBroadcaster<E>>,
        user_id: UserId,
        connection_id: String,
    ) -> Self {
        Self {
            broadcaster,
            user_id,
            connection_id,
        }
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> Drop for ConnectionGuard<E> {
    fn drop(&mut self) {
        let broadcaster = self.broadcaster;
        let user_id = self.user_id.clone();
        let conn_id = self.connection_id.clone();

        tokio::spawn(async move {
            broadcaster.unregister(&user_id, &conn_id).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use systemprompt_models::SystemEvent;

    type TestBroadcaster = GenericBroadcaster<SystemEvent>;

    fn test_user_id() -> UserId {
        UserId::new("test-user-123")
    }

    fn test_event() -> SystemEvent {
        systemprompt_models::SystemEventBuilder::heartbeat()
    }

    // ============================================================================
    // Constants Tests
    // ============================================================================

    #[test]
    fn test_heartbeat_json_format() {
        assert_eq!(HEARTBEAT_JSON, r#"{"type":"heartbeat"}"#);
    }

    #[test]
    fn test_heartbeat_json_is_valid_json() {
        let result: Result<serde_json::Value, _> = serde_json::from_str(HEARTBEAT_JSON);
        assert!(result.is_ok(), "HEARTBEAT_JSON should be valid JSON");
        let parsed = result.unwrap_or_default();
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

    // ============================================================================
    // GenericBroadcaster Construction Tests
    // ============================================================================

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

    // ============================================================================
    // Register Tests
    // ============================================================================

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

        // Should still be 1 connection since same ID was used
        assert_eq!(broadcaster.connection_count(&user_id).await, 1);
    }

    // ============================================================================
    // Unregister Tests
    // ============================================================================

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

        // Should not panic when unregistering non-existent connection
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

    // ============================================================================
    // Broadcast Tests
    // ============================================================================

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

        // Drop receiver to make sender1 fail
        drop(rx1);

        let count = broadcaster.broadcast(&user_id, test_event()).await;
        // Only conn-2 should succeed
        assert_eq!(count, 1);
        // Failed connection should be removed
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
        // user2's receiver should be empty
        assert!(rx2.try_recv().is_err());
    }

    // ============================================================================
    // Connection Count Tests
    // ============================================================================

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

    // ============================================================================
    // Connected Users Tests
    // ============================================================================

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

    // ============================================================================
    // Connection Info Tests
    // ============================================================================

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

    // ============================================================================
    // Type Alias Tests
    // ============================================================================

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

    // ============================================================================
    // ConnectionGuard Tests
    // ============================================================================

    #[tokio::test]
    async fn test_connection_guard_debug() {
        use std::sync::LazyLock;

        static TEST_BROADCASTER: Lazy<GenericBroadcaster<SystemEvent>> =
            Lazy::new(GenericBroadcaster::new);

        let guard = ConnectionGuard::new(
            &TEST_BROADCASTER,
            UserId::new("test-user"),
            "test-conn".to_string(),
        );

        let debug_str = format!("{:?}", guard);
        assert!(debug_str.contains("ConnectionGuard"));
        assert!(debug_str.contains("test-user"));
        assert!(debug_str.contains("test-conn"));
    }
}
