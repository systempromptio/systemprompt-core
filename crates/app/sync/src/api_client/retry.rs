//! Retry-policy configuration for [`crate::api_client::SyncApiClient`]:
//! exponential backoff with a configurable cap.

use std::time::Duration;

/// Bounded-exponential-backoff retry configuration used by every retryable
/// operation on [`crate::api_client::SyncApiClient`].
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first one).
    pub max_attempts: u32,
    /// Delay before the first retry.
    pub initial_delay: Duration,
    /// Upper bound on a single retry delay.
    pub max_delay: Duration,
    /// Multiplier applied to `current_delay` between attempts.
    pub exponential_base: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            exponential_base: 2,
        }
    }
}

impl RetryConfig {
    /// Compute the delay for the next attempt given the current delay,
    /// applying the configured exponential base and clamping at `max_delay`.
    pub fn next_delay(&self, current: Duration) -> Duration {
        current
            .saturating_mul(self.exponential_base)
            .min(self.max_delay)
    }
}
