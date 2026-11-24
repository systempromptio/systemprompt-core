//! Webhook broadcast surface for context events.
//!
//! Receives A2A and AG-UI webhook payloads, loads the referenced entities, and
//! fans the resulting events out to subscribed context streams. The API mounts
//! the router [`router`] builds at `/api/v1/webhook` behind the authenticated
//! gate with a user request context under the `webhook` rate scope. Handlers
//! read [`AgentHttpState`](super::AgentHttpState): the conversation
//! repositories and the member's
//! [`ContextEventPublisher`](super::ContextEventPublisher).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod broadcast_handlers;
mod context_broadcast;
pub mod error;
pub mod event_loader;
pub mod types;
pub mod validation;

use axum::Router;
use axum::routing::post;

pub use broadcast_handlers::{broadcast_a2a_event, broadcast_agui_event};
pub use context_broadcast::broadcast_context_event;
pub use types::{A2ABroadcastRequest, AgUiBroadcastRequest, WebhookRequest};

use super::AgentHttpState;

pub fn router(state: AgentHttpState) -> Router {
    Router::new()
        .route("/broadcast", post(broadcast_context_event))
        .route("/agui", post(broadcast_agui_event))
        .route("/a2a", post(broadcast_a2a_event))
        .with_state(state)
}
