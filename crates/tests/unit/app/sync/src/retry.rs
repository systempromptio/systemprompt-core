//! Tests for `RetryConfig` exponential backoff calculation.

use std::time::Duration;
use systemprompt_sync::api_client::RetryConfig;

#[test]
fn default_values_are_sensible() {
    let c = RetryConfig::default();
    assert_eq!(c.max_attempts, 5);
    assert_eq!(c.initial_delay, Duration::from_secs(2));
    assert_eq!(c.max_delay, Duration::from_secs(30));
    assert_eq!(c.exponential_base, 2);
}

#[test]
fn next_delay_doubles_then_caps() {
    let c = RetryConfig::default();
    let d1 = c.next_delay(Duration::from_secs(2));
    assert_eq!(d1, Duration::from_secs(4));
    let d2 = c.next_delay(d1);
    assert_eq!(d2, Duration::from_secs(8));
    let d3 = c.next_delay(d2);
    assert_eq!(d3, Duration::from_secs(16));
    let d4 = c.next_delay(d3);
    // 32 capped at 30
    assert_eq!(d4, Duration::from_secs(30));
    let d5 = c.next_delay(d4);
    assert_eq!(d5, Duration::from_secs(30));
}

#[test]
fn saturates_on_huge_input() {
    let c = RetryConfig::default();
    let d = c.next_delay(Duration::from_secs(u64::MAX / 2));
    assert!(d <= Duration::from_secs(30));
}

#[test]
fn custom_base_triples() {
    let c = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(100),
        exponential_base: 3,
    };
    assert_eq!(c.next_delay(Duration::from_secs(2)), Duration::from_secs(6));
}

#[test]
fn debug_and_clone() {
    let c = RetryConfig::default();
    let c2 = c;
    assert_eq!(c.max_attempts, c2.max_attempts);
    assert!(format!("{c:?}").contains("RetryConfig"));
}
