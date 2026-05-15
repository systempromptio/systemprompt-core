//! Error classification — the contract between a caller's error type and the
//! resilience primitives.

use std::time::Duration;

/// How the resilience layer should treat the result of a single attempt.
///
/// Callers implement the mapping from their own error type to this enum and
/// pass it as a `Fn(&E) -> Outcome` closure. [`Outcome::Transient`] failures
/// are retried and count toward the circuit breaker; [`Outcome::Permanent`]
/// failures fail fast but still count toward the breaker (a
/// steadily-misconfigured dependency is unhealthy regardless of whether
/// retrying would help).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The attempt succeeded — reset the breaker's failure count.
    Success,
    /// The attempt failed transiently and may succeed if retried. `retry_after`
    /// carries a server-supplied hint (e.g. a parsed `Retry-After` header).
    Transient { retry_after: Option<Duration> },
    /// The attempt failed permanently — retrying cannot help (auth,
    /// validation).
    Permanent,
}

impl Outcome {
    /// Whether this outcome should be retried.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Transient { .. })
    }
}
