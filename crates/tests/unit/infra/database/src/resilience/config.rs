//! Tests for `RetryConfig`, `BreakerConfig`, `BulkheadConfig`, and `ResilienceConfig`.

use std::time::Duration;

use systemprompt_database::resilience::config::{
    BreakerConfig, BulkheadConfig, ResilienceConfig, RetryConfig,
};

#[test]
fn retry_config_default_max_attempts() {
    assert_eq!(RetryConfig::default().max_attempts, 3);
}

#[test]
fn retry_config_default_base_delay() {
    assert_eq!(
        RetryConfig::default().base_delay,
        Duration::from_millis(200)
    );
}

#[test]
fn retry_config_default_max_delay() {
    assert_eq!(RetryConfig::default().max_delay, Duration::from_secs(10));
}

#[test]
fn retry_config_default_jitter_enabled() {
    assert!(RetryConfig::default().jitter);
}

#[test]
fn retry_config_debug() {
    let cfg = RetryConfig::default();
    let debug = format!("{:?}", cfg);
    assert!(debug.contains("RetryConfig"));
    assert!(debug.contains("max_attempts"));
}

#[test]
fn retry_config_copy() {
    let original = RetryConfig::default();
    let copy = original;
    assert_eq!(original.max_attempts, copy.max_attempts);
}

#[test]
fn breaker_config_default_failure_threshold() {
    assert_eq!(BreakerConfig::default().failure_threshold, 5);
}

#[test]
fn breaker_config_default_cooldown() {
    assert_eq!(
        BreakerConfig::default().open_cooldown,
        Duration::from_secs(30)
    );
}

#[test]
fn breaker_config_default_half_open_probes() {
    assert_eq!(BreakerConfig::default().half_open_max_probes, 1);
}

#[test]
fn breaker_config_debug() {
    let debug = format!("{:?}", BreakerConfig::default());
    assert!(debug.contains("BreakerConfig"));
}

#[test]
fn bulkhead_config_default_max_concurrent() {
    assert_eq!(BulkheadConfig::default().max_concurrent, 16);
}

#[test]
fn bulkhead_config_debug() {
    let debug = format!("{:?}", BulkheadConfig::default());
    assert!(debug.contains("BulkheadConfig"));
}

#[test]
fn bulkhead_config_copy() {
    let original = BulkheadConfig { max_concurrent: 8 };
    let copy = original;
    assert_eq!(original.max_concurrent, copy.max_concurrent);
}

#[test]
fn resilience_config_default_request_timeout() {
    assert_eq!(
        ResilienceConfig::default().request_timeout,
        Duration::from_secs(60)
    );
}

#[test]
fn resilience_config_default_stream_idle_timeout() {
    assert_eq!(
        ResilienceConfig::default().stream_idle_timeout,
        Duration::from_secs(60)
    );
}

#[test]
fn resilience_config_default_retry_nested() {
    let cfg = ResilienceConfig::default();
    assert_eq!(cfg.retry.max_attempts, 3);
}

#[test]
fn resilience_config_default_breaker_nested() {
    let cfg = ResilienceConfig::default();
    assert_eq!(cfg.breaker.failure_threshold, 5);
}

#[test]
fn resilience_config_default_bulkhead_nested() {
    let cfg = ResilienceConfig::default();
    assert_eq!(cfg.bulkhead.max_concurrent, 16);
}

#[test]
fn resilience_config_debug() {
    let debug = format!("{:?}", ResilienceConfig::default());
    assert!(debug.contains("ResilienceConfig"));
}

#[test]
fn resilience_config_copy() {
    let original = ResilienceConfig::default();
    let copy = original;
    assert_eq!(
        original.request_timeout.as_secs(),
        copy.request_timeout.as_secs()
    );
}
