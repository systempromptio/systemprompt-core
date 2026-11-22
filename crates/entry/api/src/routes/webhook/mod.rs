//! Webhook broadcast surface for context events.
//!
//! Receives A2A and AG-UI webhook payloads, loads the referenced entities, and
//! fans the resulting events out to subscribed context streams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod broadcast_handlers;
mod context_broadcast;
pub mod error;
pub mod event_loader;
pub mod types;
pub mod validation;

pub use broadcast_handlers::{broadcast_a2a_event, broadcast_agui_event};
pub use context_broadcast::broadcast_context_event;
pub use types::{A2ABroadcastRequest, AgUiBroadcastRequest, WebhookRequest};
