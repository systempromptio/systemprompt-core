use async_trait::async_trait;
use axum::response::sse::{Event, KeepAlive};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::UserId;
use tokio::sync::RwLock;

use crate::{Broadcaster, EventSender, ToSse};

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

    #[allow(clippy::significant_drop_tightening)]
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

use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent};

pub type AgUiBroadcaster = GenericBroadcaster<AgUiEvent>;
pub type A2ABroadcaster = GenericBroadcaster<A2AEvent>;
pub type ContextBroadcaster = GenericBroadcaster<ContextEvent>;
pub type AnalyticsBroadcaster = GenericBroadcaster<AnalyticsEvent>;

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
