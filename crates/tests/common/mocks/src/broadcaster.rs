use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_events::{Broadcaster, EventSender};
use systemprompt_identifiers::UserId;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct BroadcastCall<E> {
    pub user_id: UserId,
    pub event: E,
}

#[derive(Debug, Clone)]
pub struct RegisterCall {
    pub user_id: UserId,
    pub connection_id: String,
}

pub struct MockBroadcaster<E: Clone + Send> {
    broadcasts: Arc<Mutex<Vec<BroadcastCall<E>>>>,
    registrations: Arc<Mutex<Vec<RegisterCall>>>,
    unregistrations: Arc<Mutex<Vec<RegisterCall>>>,
    connection_count: usize,
    total_connections: usize,
    broadcast_return: usize,
}

impl<E: Clone + Send> MockBroadcaster<E> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            broadcasts: Arc::new(Mutex::new(Vec::new())),
            registrations: Arc::new(Mutex::new(Vec::new())),
            unregistrations: Arc::new(Mutex::new(Vec::new())),
            connection_count: 0,
            total_connections: 0,
            broadcast_return: 0,
        }
    }

    #[must_use]
    pub fn with_connection_count(mut self, count: usize) -> Self {
        self.connection_count = count;
        self
    }

    #[must_use]
    pub fn with_total_connections(mut self, count: usize) -> Self {
        self.total_connections = count;
        self
    }

    #[must_use]
    pub fn with_broadcast_return(mut self, count: usize) -> Self {
        self.broadcast_return = count;
        self
    }

    pub async fn broadcast_calls(&self) -> Vec<BroadcastCall<E>> {
        self.broadcasts.lock().await.clone()
    }

    pub async fn registration_calls(&self) -> Vec<RegisterCall> {
        self.registrations.lock().await.clone()
    }

    pub async fn unregistration_calls(&self) -> Vec<RegisterCall> {
        self.unregistrations.lock().await.clone()
    }
}

impl<E: Clone + Send> Default for MockBroadcaster<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<E: Clone + Send + Sync + 'static> Broadcaster for MockBroadcaster<E> {
    type Event = E;

    async fn register(&self, user_id: &UserId, connection_id: &str, _sender: EventSender) {
        self.registrations.lock().await.push(RegisterCall {
            user_id: user_id.clone(),
            connection_id: connection_id.to_string(),
        });
    }

    async fn unregister(&self, user_id: &UserId, connection_id: &str) {
        self.unregistrations.lock().await.push(RegisterCall {
            user_id: user_id.clone(),
            connection_id: connection_id.to_string(),
        });
    }

    async fn broadcast(&self, user_id: &UserId, event: Self::Event) -> usize {
        self.broadcasts.lock().await.push(BroadcastCall {
            user_id: user_id.clone(),
            event,
        });
        self.broadcast_return
    }

    async fn connection_count(&self, _user_id: &UserId) -> usize {
        self.connection_count
    }

    async fn total_connections(&self) -> usize {
        self.total_connections
    }
}
