//! Unit tests for RetryConfiguration, TimeoutConfiguration, TimeoutType, and delay calculation

use std::time::Duration;
use systemprompt_agent::services::shared::resilience::{
    RetryConfiguration, TimeoutConfiguration, TimeoutType,
};

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
fn test_retry_configuration_copy() {
    let config = RetryConfiguration::default();
    let copied = config;

    assert_eq!(copied.max_attempts, 3);
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
fn test_timeout_configuration_default() {
    let config = TimeoutConfiguration::default();

    assert_eq!(config.default, Duration::from_secs(30));
    assert_eq!(config.connect, Duration::from_secs(10));
    assert_eq!(config.read, Duration::from_secs(30));
    assert_eq!(config.write, Duration::from_secs(30));
}

#[test]
fn test_timeout_configuration_custom() {
    let config = TimeoutConfiguration {
        default: Duration::from_secs(60),
        connect: Duration::from_secs(5),
        read: Duration::from_secs(120),
        write: Duration::from_secs(90),
    };

    assert_eq!(config.default, Duration::from_secs(60));
    assert_eq!(config.connect, Duration::from_secs(5));
    assert_eq!(config.read, Duration::from_secs(120));
    assert_eq!(config.write, Duration::from_secs(90));
}

#[test]
fn test_timeout_configuration_clone() {
    let config = TimeoutConfiguration::default();
    let cloned = config;

    assert_eq!(cloned.default, config.default);
    assert_eq!(cloned.connect, config.connect);
}

#[test]
fn test_timeout_configuration_debug() {
    let config = TimeoutConfiguration::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("TimeoutConfiguration"));
    assert!(debug_str.contains("default"));
    assert!(debug_str.contains("connect"));
}

#[test]
fn test_timeout_type_connect() {
    let timeout_type = TimeoutType::Connect;
    let debug_str = format!("{:?}", timeout_type);
    assert!(debug_str.contains("Connect"));
}

#[test]
fn test_timeout_type_read() {
    let timeout_type = TimeoutType::Read;
    let debug_str = format!("{:?}", timeout_type);
    assert!(debug_str.contains("Read"));
}

#[test]
fn test_timeout_type_write() {
    let timeout_type = TimeoutType::Write;
    let debug_str = format!("{:?}", timeout_type);
    assert!(debug_str.contains("Write"));
}

#[test]
fn test_timeout_type_default() {
    let timeout_type = TimeoutType::Default;
    let debug_str = format!("{:?}", timeout_type);
    assert!(debug_str.contains("Default"));
}

#[test]
fn test_timeout_type_clone() {
    let original = TimeoutType::Connect;
    let cloned = original;

    match cloned {
        TimeoutType::Connect => {}
        _ => panic!("Expected Connect variant"),
    }
}

#[test]
fn test_timeout_type_copy() {
    let original = TimeoutType::Read;
    let copied = original;

    match copied {
        TimeoutType::Read => {}
        _ => panic!("Expected Read variant"),
    }
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
            let delay = config.initial_delay.saturating_mul(config.exponential_base.pow(i));
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

#[test]
fn test_timeout_selection() {
    let config = TimeoutConfiguration::default();

    let get_timeout = |timeout_type: TimeoutType| -> Duration {
        match timeout_type {
            TimeoutType::Connect => config.connect,
            TimeoutType::Read => config.read,
            TimeoutType::Write => config.write,
            TimeoutType::Default => config.default,
        }
    };

    assert_eq!(get_timeout(TimeoutType::Connect), Duration::from_secs(10));
    assert_eq!(get_timeout(TimeoutType::Read), Duration::from_secs(30));
    assert_eq!(get_timeout(TimeoutType::Write), Duration::from_secs(30));
    assert_eq!(get_timeout(TimeoutType::Default), Duration::from_secs(30));
}
