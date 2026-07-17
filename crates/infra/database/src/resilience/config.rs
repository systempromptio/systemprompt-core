//! Runtime configuration for the resilience primitives.
//!
//! These are the in-memory form used by [`super::guard::ResilienceGuard`].
//! Callers that load configuration from disk (e.g. `systemprompt-models` config
//! structs in milliseconds) translate into these `Duration`-typed structs at
//! construction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Counts the first try, so `1` disables retries.
    pub max_attempts: u32,
    /// Doubles each subsequent attempt.
    pub base_delay: Duration,
    pub max_delay: Duration,
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

#[derive(Debug, Clone, Copy)]
pub struct BreakerConfig {
    /// Consecutive (not cumulative) failures that trip the breaker open.
    pub failure_threshold: u32,
    pub open_cooldown: Duration,
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

#[derive(Debug, Clone, Copy)]
pub struct BulkheadConfig {
    pub max_concurrent: usize,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self { max_concurrent: 16 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResilienceConfig {
    /// Per-attempt (not whole-call) timeout; non-streaming only.
    pub request_timeout: Duration,
    /// Max gap between two chunks before a stream is aborted.
    pub stream_idle_timeout: Duration,
    pub retry: RetryConfig,
    pub breaker: BreakerConfig,
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

impl From<&systemprompt_models::services::ResilienceSettings> for ResilienceConfig {
    /// Count fields are clamped to a minimum of `1`: a zero
    /// attempt/probe/permit budget would deadlock every guarded call.
    fn from(settings: &systemprompt_models::services::ResilienceSettings) -> Self {
        Self {
            request_timeout: Duration::from_millis(settings.request_timeout_ms),
            stream_idle_timeout: Duration::from_millis(settings.stream_idle_timeout_ms),
            retry: RetryConfig {
                max_attempts: settings.retry_attempts.max(1),
                base_delay: Duration::from_millis(settings.retry_base_delay_ms),
                max_delay: Duration::from_millis(settings.retry_max_delay_ms),
                jitter: true,
            },
            breaker: BreakerConfig {
                failure_threshold: settings.breaker_failure_threshold.max(1),
                open_cooldown: Duration::from_millis(settings.breaker_open_cooldown_ms),
                half_open_max_probes: settings.breaker_half_open_probes.max(1),
            },
            bulkhead: BulkheadConfig {
                max_concurrent: settings.max_concurrent.max(1),
            },
        }
    }
}
