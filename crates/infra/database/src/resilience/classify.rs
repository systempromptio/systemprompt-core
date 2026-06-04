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
    Success,
    Transient { retry_after: Option<Duration> },
    Permanent,
}

impl Outcome {
    #[must_use]
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Transient { .. })
    }
}
