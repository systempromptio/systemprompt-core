//! Retry-policy configuration for [`crate::api_client::SyncApiClient`]:
//! exponential backoff with a configurable cap.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
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
    pub fn next_delay(&self, current: Duration) -> Duration {
        current
            .saturating_mul(self.exponential_base)
            .min(self.max_delay)
    }
}
