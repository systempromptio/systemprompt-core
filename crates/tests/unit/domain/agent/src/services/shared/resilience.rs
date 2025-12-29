//! Unit tests for resilience utilities
//!
//! Tests cover:
//! - RetryConfiguration default values
//! - TimeoutConfiguration default values
//! - TimeoutType enum
//! - Delay calculation logic

use std::time::Duration;
use systemprompt_core_agent::services::shared::resilience::{
    RetryConfiguration, TimeoutConfiguration, TimeoutType,
};

// ============================================================================
// RetryConfiguration Tests
// ============================================================================

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

// ============================================================================
// TimeoutConfiguration Tests
// ============================================================================

#[test]
fn test_timeout_configuration_default() {
    let config = TimeoutConfiguration::default();

    assert_eq!(config.default_timeout, Duration::from_secs(30));
    assert_eq!(config.connect_timeout, Duration::from_secs(10));
    assert_eq!(config.read_timeout, Duration::from_secs(30));
    assert_eq!(config.write_timeout, Duration::from_secs(30));
}

#[test]
fn test_timeout_configuration_custom() {
    let config = TimeoutConfiguration {
        default_timeout: Duration::from_secs(60),
        connect_timeout: Duration::from_secs(5),
        read_timeout: Duration::from_secs(120),
        write_timeout: Duration::from_secs(90),
    };

    assert_eq!(config.default_timeout, Duration::from_secs(60));
    assert_eq!(config.connect_timeout, Duration::from_secs(5));
    assert_eq!(config.read_timeout, Duration::from_secs(120));
    assert_eq!(config.write_timeout, Duration::from_secs(90));
}

#[test]
fn test_timeout_configuration_clone() {
    let config = TimeoutConfiguration::default();
    let cloned = config;

    assert_eq!(cloned.default_timeout, config.default_timeout);
    assert_eq!(cloned.connect_timeout, config.connect_timeout);
}

#[test]
fn test_timeout_configuration_debug() {
    let config = TimeoutConfiguration::default();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("TimeoutConfiguration"));
    assert!(debug_str.contains("default_timeout"));
    assert!(debug_str.contains("connect_timeout"));
}

// ============================================================================
// TimeoutType Tests
// ============================================================================

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

// ============================================================================
// Delay Calculation Tests
// ============================================================================

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

// ============================================================================
// Integration-like Tests (synchronous parts)
// ============================================================================

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
            TimeoutType::Connect => config.connect_timeout,
            TimeoutType::Read => config.read_timeout,
            TimeoutType::Write => config.write_timeout,
            TimeoutType::Default => config.default_timeout,
        }
    };

    assert_eq!(get_timeout(TimeoutType::Connect), Duration::from_secs(10));
    assert_eq!(get_timeout(TimeoutType::Read), Duration::from_secs(30));
    assert_eq!(get_timeout(TimeoutType::Write), Duration::from_secs(30));
    assert_eq!(get_timeout(TimeoutType::Default), Duration::from_secs(30));
}

// ============================================================================
// Async Retry Operation Tests
// ============================================================================

use systemprompt_core_agent::services::shared::resilience::{
    execute_with_timeout, execute_with_custom_timeout, retry_operation, retry_operation_with_backoff,
};
use systemprompt_core_agent::services::shared::error::AgentServiceError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_retry_operation_succeeds_first_try() {
    let config = RetryConfiguration {
        max_attempts: 3,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        exponential_base: 2,
    };

    let result = retry_operation(|| async { Ok::<i32, AgentServiceError>(42) }, config).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_retry_operation_fails_all_attempts() {
    let config = RetryConfiguration {
        max_attempts: 2,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        exponential_base: 2,
    };

    let result = retry_operation(
        || async { Err::<i32, AgentServiceError>(AgentServiceError::Network("fail".to_string())) },
        config,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_retry_operation_succeeds_after_retries() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let config = RetryConfiguration {
        max_attempts: 3,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        exponential_base: 2,
    };

    let result = retry_operation(
        || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(AgentServiceError::Network("retry".to_string()))
                } else {
                    Ok(42)
                }
            }
        },
        config,
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_retry_operation_with_backoff_succeeds() {
    let result = retry_operation_with_backoff(
        || async { Ok::<String, AgentServiceError>("success".to_string()) },
        3,
        Duration::from_millis(1),
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
}

#[tokio::test]
async fn test_retry_operation_with_backoff_fails() {
    let result = retry_operation_with_backoff(
        || async { Err::<String, AgentServiceError>(AgentServiceError::Timeout(100)) },
        2,
        Duration::from_millis(1),
    )
    .await;

    assert!(result.is_err());
}

// ============================================================================
// Async Timeout Operation Tests
// ============================================================================

#[tokio::test]
async fn test_execute_with_timeout_succeeds() {
    let result = execute_with_timeout(Duration::from_secs(1), async {
        Ok::<i32, AgentServiceError>(42)
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_execute_with_timeout_times_out() {
    let result = execute_with_timeout(Duration::from_millis(1), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<i32, AgentServiceError>(42)
    })
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentServiceError::Timeout(ms) => assert_eq!(ms, 1),
        _ => panic!("Expected Timeout error"),
    }
}

#[tokio::test]
async fn test_execute_with_timeout_propagates_error() {
    let result = execute_with_timeout(Duration::from_secs(1), async {
        Err::<i32, AgentServiceError>(AgentServiceError::Database("db error".to_string()))
    })
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentServiceError::Database(msg) => assert_eq!(msg, "db error"),
        _ => panic!("Expected Database error"),
    }
}

#[tokio::test]
async fn test_execute_with_custom_timeout_connect() {
    let config = TimeoutConfiguration {
        default_timeout: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(30),
        write_timeout: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Connect, async { Ok::<i32, AgentServiceError>(1) }).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_custom_timeout_read() {
    let config = TimeoutConfiguration {
        default_timeout: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(10),
        read_timeout: Duration::from_secs(1),
        write_timeout: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Read, async { Ok::<i32, AgentServiceError>(2) }).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_custom_timeout_write() {
    let config = TimeoutConfiguration {
        default_timeout: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(10),
        read_timeout: Duration::from_secs(30),
        write_timeout: Duration::from_secs(1),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Write, async { Ok::<i32, AgentServiceError>(3) }).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_with_custom_timeout_default() {
    let config = TimeoutConfiguration {
        default_timeout: Duration::from_secs(1),
        connect_timeout: Duration::from_secs(10),
        read_timeout: Duration::from_secs(30),
        write_timeout: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Default, async { Ok::<i32, AgentServiceError>(4) }).await;

    assert!(result.is_ok());
}
