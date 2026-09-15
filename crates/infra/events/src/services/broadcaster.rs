//! Generic SSE broadcaster.
//!
//! [`GenericBroadcaster`] holds a `RwLock<HashMap<UserId, HashMap<ConnId,
//! Sender>>>` and is parameterised over the payload type via the [`ToSse`]
//! trait. Concrete type aliases (`A2ABroadcaster`, `AgUiBroadcaster`, etc.)
//! pick the event kind so that callers never need to spell out the generic.
//!
//! The registry is guarded by a `std::sync::RwLock`: every critical section
//! is a map lookup with no await inside, so `ConnectionGuard::drop` can
//! unregister without a runtime and the async trait methods resolve eagerly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::{Event, KeepAlive};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;
use systemprompt_identifiers::{ConnectionId, UserId};
use tokio::sync::mpsc::error::TrySendError;

use crate::{Broadcaster, EventSender, ToSse};

pub const HEARTBEAT_JSON: &str = r#"{"type":"heartbeat"}"#;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

pub fn standard_keep_alive() -> KeepAlive {
    KeepAlive::new()
        .interval(HEARTBEAT_INTERVAL)
        .event(Event::default().event("heartbeat").data(HEARTBEAT_JSON))
}

type Registry = HashMap<UserId, HashMap<ConnectionId, EventSender>>;

pub struct GenericBroadcaster<E: ToSse + Clone + Send + Sync> {
    connections: Arc<RwLock<Registry>>,
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
    pub const MAX_CONNECTIONS_PER_USER: usize = 10;

    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            _phantom: PhantomData,
        }
    }

    pub fn connected_users(&self) -> Vec<UserId> {
        self.read().keys().cloned().collect()
    }

    pub fn connection_info(&self) -> (usize, usize) {
        let connections = self.read();
        (
            connections.len(),
            connections.values().map(HashMap::len).sum(),
        )
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Registry> {
        self.connections
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Registry> {
        self.connections
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn remove_connection(&self, user_id: &UserId, connection_id: &ConnectionId) {
        let mut connections = self.write();
        if let Some(user_connections) = connections.get_mut(user_id) {
            user_connections.remove(connection_id);
            if user_connections.is_empty() {
                connections.remove(user_id);
            }
        }
    }
}

impl<E: ToSse + Clone + Send + Sync> Default for GenericBroadcaster<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> Broadcaster for GenericBroadcaster<E> {
    type Event = E;

    fn register(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
        sender: EventSender,
    ) -> impl Future<Output = bool> + Send {
        std::future::ready(self.register_now(user_id, connection_id, sender))
    }

    fn unregister(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
    ) -> impl Future<Output = ()> + Send {
        self.remove_connection(user_id, connection_id);
        std::future::ready(())
    }

    fn broadcast(
        &self,
        user_id: &UserId,
        event: Self::Event,
    ) -> impl Future<Output = usize> + Send {
        std::future::ready(self.broadcast_now(user_id, &event))
    }

    fn connection_count(&self, user_id: &UserId) -> impl Future<Output = usize> + Send {
        std::future::ready(self.read().get(user_id).map_or(0, HashMap::len))
    }

    fn total_connections(&self) -> impl Future<Output = usize> + Send {
        std::future::ready(self.read().values().map(HashMap::len).sum())
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> GenericBroadcaster<E> {
    fn register_now(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
        sender: EventSender,
    ) -> bool {
        let mut connections = self.write();
        let user_connections = connections.entry(user_id.clone()).or_default();
        if user_connections.len() >= Self::MAX_CONNECTIONS_PER_USER
            && !user_connections.contains_key(connection_id)
        {
            drop(connections);
            tracing::warn!(
                user_id = %user_id,
                max = Self::MAX_CONNECTIONS_PER_USER,
                "rejecting SSE registration: per-user connection cap reached"
            );
            return false;
        }
        user_connections.insert(connection_id.clone(), sender);
        drop(connections);
        true
    }

    fn broadcast_now(&self, user_id: &UserId, event: &E) -> usize {
        let sse_event: Event = match event.to_sse() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, event_type = ?std::any::type_name_of_val(&event), "Failed to serialize SSE event");
                return 0;
            },
        };

        let senders: Vec<(ConnectionId, EventSender)> = {
            let connections = self.read();
            match connections.get(user_id) {
                Some(user_connections) => user_connections
                    .iter()
                    .map(|(id, sender)| (id.clone(), sender.clone()))
                    .collect(),
                None => return 0,
            }
        };

        let mut successful = 0;
        // Why: a full channel is a consumer that stopped reading, not a dead
        // one. Its sender is dropped so the stream ends and the client
        // reconnects, instead of silently receiving heartbeats only.
        for (conn_id, sender) in senders {
            match sender.try_send(Ok(sse_event.clone())) {
                Ok(()) => successful += 1,
                Err(TrySendError::Closed(_)) => self.remove_connection(user_id, &conn_id),
                Err(TrySendError::Full(_)) => {
                    tracing::warn!(
                        user_id = %user_id,
                        connection_id = %conn_id,
                        "SSE consumer is not draining its channel; closing its stream"
                    );
                    self.remove_connection(user_id, &conn_id);
                },
            }
        }

        successful
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
    connection_id: ConnectionId,
}

impl<E: ToSse + Clone + Send + Sync + 'static> std::fmt::Debug for ConnectionGuard<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionGuard")
            .field("user_id", &self.user_id)
            .field("connection_id", &self.connection_id)
            .field("broadcaster", &"<LazyLock<GenericBroadcaster>>")
            .finish()
    }
}

impl<E: ToSse + Clone + Send + Sync + 'static> ConnectionGuard<E> {
    #[must_use]
    pub fn new(
        broadcaster: &'static std::sync::LazyLock<GenericBroadcaster<E>>,
        user_id: UserId,
        connection_id: ConnectionId,
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
        self.broadcaster
            .remove_connection(&self.user_id, &self.connection_id);
    }
}
