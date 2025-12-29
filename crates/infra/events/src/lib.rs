pub mod services;

use async_trait::async_trait;
use axum::response::sse::Event;
use systemprompt_identifiers::UserId;
use tokio::sync::mpsc::UnboundedSender;

pub type EventSender = UnboundedSender<Result<Event, std::convert::Infallible>>;

pub use systemprompt_models::events::ToSse;

#[async_trait]
pub trait Broadcaster: Send + Sync {
    type Event: Clone + Send;

    async fn register(&self, user_id: &UserId, connection_id: &str, sender: EventSender);
    async fn unregister(&self, user_id: &UserId, connection_id: &str);
    async fn broadcast(&self, user_id: &UserId, event: Self::Event) -> usize;
    async fn connection_count(&self, user_id: &UserId) -> usize;
    async fn total_connections(&self) -> usize;
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: systemprompt_models::AgUiEvent,
    ) -> (usize, usize);
    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: systemprompt_models::A2AEvent,
    ) -> (usize, usize);
    async fn broadcast_system(
        &self,
        user_id: &UserId,
        event: systemprompt_models::SystemEvent,
    ) -> usize;
    async fn broadcast_context(
        &self,
        user_id: &UserId,
        event: systemprompt_models::ContextEvent,
    ) -> usize;
}

pub use services::{
    standard_keep_alive, A2ABroadcaster, AgUiBroadcaster, ConnectionGuard, ContextBroadcaster,
    EventRouter, GenericBroadcaster, A2A_BROADCASTER, AGUI_BROADCASTER, CONTEXT_BROADCASTER,
    HEARTBEAT_INTERVAL, HEARTBEAT_JSON,
};
