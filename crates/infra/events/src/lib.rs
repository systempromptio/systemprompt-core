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

/// Mpsc sender wired to a single SSE connection.
///
/// The error half is `Infallible` because the SSE response stream cannot
/// generate `Result::Err` items — connection failures surface via channel
/// close.
pub type EventSender = Sender<Result<Event, std::convert::Infallible>>;

/// Default capacity of per-connection SSE buffers.
///
/// Sized to absorb bursts during template rendering and tool execution
/// without forcing producers to block.
pub const SSE_BUFFER: usize = 1024;

pub use error::{EventError, EventResult};
pub use sse::ToSse;

/// Trait implemented by per-event-kind broadcasters.
///
/// Implementations are expected to be cheap to clone (typically wrapping an
/// `Arc<RwLock<...>>`) and safe to share across tasks.
///
/// `#[async_trait]` is retained because callers store broadcasters behind
/// `dyn Broadcaster<Event = …>` trait objects (in particular the static
/// [`services::EventRouter`] dispatch tables); native `async fn` in traits
/// is not yet `dyn`-compatible.
#[async_trait]
pub trait Broadcaster: Send + Sync {
    /// Concrete event payload routed by this broadcaster.
    type Event: Clone + Send;

    /// Registers a new SSE connection for `user_id`.
    ///
    /// Subsequent calls to [`broadcast`](Self::broadcast) for the same user
    /// will fan out to `sender` until [`unregister`](Self::unregister) is
    /// called or the channel is closed.
    async fn register(&self, user_id: &UserId, connection_id: &str, sender: EventSender);

    /// Removes a previously registered SSE connection.
    async fn unregister(&self, user_id: &UserId, connection_id: &str);

    /// Fans `event` out to every connection registered under `user_id`.
    ///
    /// Returns the number of connections that successfully accepted the
    /// event. Connections whose channel is full or closed are evicted from
    /// the registry.
    async fn broadcast(&self, user_id: &UserId, event: Self::Event) -> usize;

    /// Returns the number of currently registered connections for `user_id`.
    async fn connection_count(&self, user_id: &UserId) -> usize;

    /// Returns the total number of registered connections across all users.
    async fn total_connections(&self) -> usize;
}

pub use services::{
    A2A_BROADCASTER, A2ABroadcaster, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, AgUiBroadcaster,
    AnalyticsBroadcaster, CONTEXT_BROADCASTER, ConnectionGuard, ContextBroadcaster, EventRouter,
    GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON, standard_keep_alive,
};
