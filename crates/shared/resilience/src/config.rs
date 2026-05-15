//! Runtime configuration for the resilience primitives.
//!
//! These are the in-memory form used by [`crate::guard::ResilienceGuard`].
//! Callers that load configuration from disk (e.g. `systemprompt-models` config
//! structs in milliseconds) translate into these `Duration`-typed structs at
//! construction.

use std::time::Duration;

/// Bounded-retry policy with exponential backoff and jitter.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of attempts, including the first. `1` disables retries.
    pub max_attempts: u32,
    /// Backoff before the first retry; doubles each subsequent attempt.
    pub base_delay: Duration,
    /// Upper bound on a single backoff delay.
    pub max_delay: Duration,
    /// Whether to apply full jitter to each backoff delay.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            jitter: true,
        }
    }
}

/// Circuit-breaker policy.
#[derive(Debug, Clone, Copy)]
pub struct BreakerConfig {
    /// Consecutive failures that trip the breaker from `Closed` to `Open`.
    pub failure_threshold: u32,
    /// How long the breaker stays `Open` before allowing a half-open probe.
    pub open_cooldown: Duration,
    /// Concurrent probes permitted while `HalfOpen`.
    pub half_open_max_probes: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_cooldown: Duration::from_secs(30),
            half_open_max_probes: 1,
        }
    }
}

/// Concurrency-limit (bulkhead) policy.
#[derive(Debug, Clone, Copy)]
pub struct BulkheadConfig {
    /// Maximum number of in-flight calls; further calls are rejected.
    pub max_concurrent: usize,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self { max_concurrent: 16 }
    }
}

/// The full resilience policy applied to one logical dependency.
#[derive(Debug, Clone, Copy)]
pub struct ResilienceConfig {
    /// Timeout applied to each individual attempt of a non-streaming call.
    pub request_timeout: Duration,
    /// Maximum gap between two chunks of a streaming response before it is
    /// aborted.
    pub stream_idle_timeout: Duration,
    /// Retry policy.
    pub retry: RetryConfig,
    /// Circuit-breaker policy.
    pub breaker: BreakerConfig,
    /// Bulkhead policy.
    pub bulkhead: BulkheadConfig,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            stream_idle_timeout: Duration::from_secs(60),
            retry: RetryConfig::default(),
            breaker: BreakerConfig::default(),
            bulkhead: BulkheadConfig::default(),
        }
    }
}
