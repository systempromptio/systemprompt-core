//! Error types raised by the event-broadcasting infrastructure.
//!
//! [`EventError`] is the public, `thiserror`-derived enum returned from every
//! fallible operation in the crate. It composes via `#[from]` with
//! `serde_json::Error` so callers can compose it into larger error enums
//! without wrapping by hand.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("event channel saturated for {target}")]
    ChannelFull { target: String },
}

pub type EventResult<T> = Result<T, EventError>;
