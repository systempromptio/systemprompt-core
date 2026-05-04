//! Error types raised by the event-broadcasting infrastructure.
//!
//! [`EventError`] is the public, `thiserror`-derived enum returned from every
//! fallible operation in the crate. It composes via `#[from]` with
//! `serde_json::Error` so callers can compose it into larger error enums
//! without wrapping by hand.

use thiserror::Error;

/// Errors raised when serializing or dispatching events to subscribers.
#[derive(Debug, Error)]
pub enum EventError {
    /// The event payload could not be serialized to JSON before being framed
    /// as an SSE record.
    #[error("event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The configured channel buffer was exhausted and the event was dropped
    /// rather than block the broadcaster.
    #[error("event channel saturated for {target}")]
    ChannelFull {
        /// Human-readable identifier of the saturated channel (typically a
        /// user id or connection id).
        target: String,
    },
}

/// Convenience [`Result`] alias parameterised on [`EventError`].
pub type EventResult<T> = Result<T, EventError>;
