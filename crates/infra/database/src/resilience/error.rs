//! The error type the resilience layer itself produces.

use std::time::Duration;

/// A failure surfaced by the resilience layer wrapping a caller's operation.
///
/// [`ResilienceError::Inner`] carries the caller's own error `E` unchanged
/// (after retries are exhausted). The other variants are produced by the guard
/// itself and the caller is expected to map them into its domain error enum.
#[derive(Debug, thiserror::Error)]
pub enum ResilienceError<E> {
    /// The circuit breaker is open; the call was rejected without being
    /// attempted.
    #[error("circuit breaker '{key}' is open; failing fast")]
    CircuitOpen { key: String },

    /// The bulkhead is saturated; the call was rejected to protect capacity.
    #[error("bulkhead '{key}' is saturated ({limit} concurrent permits in use)")]
    BulkheadFull { key: String, limit: usize },

    /// The operation exceeded its per-attempt timeout on every retry.
    #[error("operation timed out after {after:?}")]
    Timeout { after: Duration },

    /// The caller's operation failed (retries exhausted, or a permanent
    /// failure).
    #[error(transparent)]
    Inner(#[from] E),
}
