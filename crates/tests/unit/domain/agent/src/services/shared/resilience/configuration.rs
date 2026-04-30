//! Unit tests for RetryConfiguration and delay calculation

use std::time::Duration;
use systemprompt_agent::services::shared::resilience::RetryConfiguration;

#[test]
fn test_retry_configuration_default() {
    let config = RetryConfiguration::default();

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.initial_delay, Duration::from_millis(100));
    assert_eq!(config.max_delay, Duration::from_secs(10));
    assert_eq!(config.exponential_base, 2);
}

#[test]
fn test_retry_configuration_custom() {
    let config = RetryConfiguration {
        max_attempts: 5,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(30),
        exponential_base: 3,
    };

    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.initial_delay, Duration::from_millis(500));
    assert_eq!(config.max_delay, Duration::from_secs(30));
    assert_eq!(config.exponential_base, 3);
}

#[test]
fn test_retry_configuration_clone() {
    let config = RetryConfiguration::default();
    let cloned = config;

    assert_eq!(cloned.max_attempts, config.max_attempts);
    assert_eq!(cloned.initial_delay, config.initial_delay);
}

#[test]
fn test_retry_configuration_debug() {
    let config = RetryConfiguration::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("RetryConfiguration"));
    assert!(debug_str.contains("max_attempts"));
    assert!(debug_str.contains("initial_delay"));
}

#[test]
fn test_exponential_delay_doubles() {
    let initial = Duration::from_millis(100);
    let doubled = initial.saturating_mul(2);

    assert_eq!(doubled, Duration::from_millis(200));
}

#[test]
fn test_exponential_delay_triples() {
    let initial = Duration::from_millis(100);
    let tripled = initial.saturating_mul(3);

    assert_eq!(tripled, Duration::from_millis(300));
}

#[test]
fn test_delay_capped_at_max() {
    let current = Duration::from_secs(8);
    let max = Duration::from_secs(10);
    let next = current.saturating_mul(2);

    let capped = if next > max { max } else { next };

    assert_eq!(capped, max);
}

#[test]
fn test_delay_below_max() {
    let current = Duration::from_secs(3);
    let max = Duration::from_secs(10);
    let next = current.saturating_mul(2);

    let capped = if next > max { max } else { next };

    assert_eq!(capped, Duration::from_secs(6));
}

#[test]
fn test_saturating_mul_prevents_overflow() {
    let large = Duration::from_secs(u64::MAX / 2);
    let result = large.saturating_mul(3);

    assert_eq!(result, Duration::MAX);
}

#[test]
fn test_retry_config_progression() {
    let config = RetryConfiguration::default();

    let delays: Vec<Duration> = (0..config.max_attempts)
        .map(|i| {
            let delay = config
                .initial_delay
                .saturating_mul(config.exponential_base.pow(i));
            if delay > config.max_delay {
                config.max_delay
            } else {
                delay
            }
        })
        .collect();

    assert_eq!(delays[0], Duration::from_millis(100));
    assert_eq!(delays[1], Duration::from_millis(200));
    assert_eq!(delays[2], Duration::from_millis(400));
}
