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

pub use services::{
    standard_keep_alive, A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ConnectionGuard,
    ContextBroadcaster, EventRouter, GenericBroadcaster, A2A_BROADCASTER, AGUI_BROADCASTER,
    ANALYTICS_BROADCASTER, CONTEXT_BROADCASTER, HEARTBEAT_INTERVAL, HEARTBEAT_JSON,
};
