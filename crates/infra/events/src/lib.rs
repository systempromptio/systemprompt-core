pub mod services;
mod sse;

use async_trait::async_trait;
use axum::response::sse::Event;
use systemprompt_identifiers::UserId;
use tokio::sync::mpsc::UnboundedSender;

pub type EventSender = UnboundedSender<Result<Event, std::convert::Infallible>>;

pub use sse::ToSse;

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
    A2A_BROADCASTER, A2ABroadcaster, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, AgUiBroadcaster,
    AnalyticsBroadcaster, CONTEXT_BROADCASTER, ConnectionGuard, ContextBroadcaster, EventRouter,
    GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON, standard_keep_alive,
};
