//! The error type the resilience layer itself produces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ResilienceError<E> {
    #[error("circuit breaker '{key}' is open; failing fast")]
    CircuitOpen { key: String },

    #[error("bulkhead '{key}' is saturated ({limit} concurrent permits in use)")]
    BulkheadFull { key: String, limit: usize },

    #[error("operation timed out after {after:?}")]
    Timeout { after: Duration },

    #[error(transparent)]
    Inner(#[from] E),
}
