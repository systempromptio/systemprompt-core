//! Webhook broadcast surface for context events.
//!
//! Receives A2A and AG-UI webhook payloads, loads the referenced entities, and
//! fans the resulting events out to subscribed context streams.

mod broadcast_handlers;
mod context_broadcast;
mod error;
mod event_loader;
mod types;
mod validation;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::error::LoadEventError;
    pub use super::event_loader::{load_execution_step, load_task_created};
    pub use super::types::AgUiWebhookData;
    pub use super::validation::{sanitize_payload, validate_json_serializable};
}

pub use broadcast_handlers::{broadcast_a2a_event, broadcast_agui_event};
pub use context_broadcast::broadcast_context_event;
pub use types::{A2ABroadcastRequest, AgUiBroadcastRequest, WebhookRequest};
