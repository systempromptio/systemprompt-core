//! Error types raised by the event-broadcasting infrastructure.
//!
//! [`EventError`] is the public, `thiserror`-derived enum returned from every
//! fallible operation in the crate. It composes via `#[from]` with
//! `serde_json::Error` so callers can compose it into larger error enums
//! without wrapping by hand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("event serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("event channel saturated for {target}")]
    ChannelFull { target: String },
}

pub type EventResult<T> = Result<T, EventError>;

/// Why a routed event did not reach the cross-replica outbox.
#[derive(Debug, Error)]
pub enum RelayError {
    #[error("event serialization failed for outbox channel {channel}: {source}")]
    Serialize {
        channel: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to persist outbox row on channel {channel}: {source}")]
    Persist {
        channel: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("outbox row persisted but NOTIFY failed: {0}")]
    Notify(#[source] sqlx::Error),
}

/// Whether a routed event was handed to the other replicas.
#[derive(Debug)]
pub enum RelayOutcome {
    NotInstalled,
    Relayed,
    Failed(RelayError),
}

/// Local fan-out counts plus the cross-replica relay result of one `route_*`.
#[derive(Debug)]
#[must_use = "a failed relay is only visible through this outcome"]
pub struct RouteOutcome<L> {
    pub local: L,
    pub relay: RelayOutcome,
}

impl<L> RouteOutcome<L> {
    pub fn into_local_logged(self) -> L {
        if let RelayOutcome::Failed(error) = &self.relay {
            tracing::warn!(error = %error, "event routed locally but not relayed to other replicas");
        }
        self.local
    }
}
