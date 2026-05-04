//! Server-Sent Events (SSE) broadcasting infrastructure for systemprompt.io.
//!
//! This crate hosts the in-process event bus that fans out A2A, AG-UI,
//! analytics, and context events to per-user SSE connections. It is shared
//! between the HTTP API entry crate and the runtime layer so that any
//! component holding a [`UserId`] can publish typed events without knowing
//! about the wire format.
//!
//! # Modules
//!
//! - [`services`] — the [`GenericBroadcaster`] implementation, the per-event
//!   broadcaster type aliases, and the static [`EventRouter`].
//! - [`sse`] — the [`ToSse`] trait and `serde`-driven implementations that
//!   convert [`systemprompt_models`] event types into `axum` SSE records.
//! - [`error`] — the public [`EventError`] / [`EventResult`] surface.
//!
//! # Feature flags
//!
//! This crate has no Cargo features; everything compiles by default.
//!
//! # Example
//!
//! ```no_run
//! use systemprompt_events::{A2A_BROADCASTER, Broadcaster};
//! use systemprompt_identifiers::UserId;
//! use systemprompt_models::A2AEvent;
//!
//! # async fn demo(event: A2AEvent) {
//! let user_id = UserId::new("user_abc");
//! let delivered = A2A_BROADCASTER.broadcast(&user_id, event).await;
//! tracing::info!(delivered, "A2A event fanned out");
//! # }
//! ```

pub mod error;
pub mod services;
pub mod sse;

use async_trait::async_trait;
use axum::response::sse::Event;
use systemprompt_identifiers::UserId;
use tokio::sync::mpsc::Sender;

pub type EventSender = Sender<Result<Event, std::convert::Infallible>>;

pub const SSE_BUFFER: usize = 1024;

pub use error::{EventError, EventResult};
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
