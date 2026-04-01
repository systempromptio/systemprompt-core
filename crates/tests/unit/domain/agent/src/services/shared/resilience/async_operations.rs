//! Async tests for retry_operation, retry_operation_with_backoff, and timeout execution

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use systemprompt_agent::services::shared::error::AgentServiceError;
use systemprompt_agent::services::shared::resilience::{
    execute_with_custom_timeout, execute_with_timeout, retry_operation,
    retry_operation_with_backoff, RetryConfiguration, TimeoutConfiguration, TimeoutType,
};

#[tokio::test]
async fn test_retry_operation_succeeds_first_try() {
    let config = RetryConfiguration {
        max_attempts: 3,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        exponential_base: 2,
    };

    let result = retry_operation(|| async { Ok::<i32, AgentServiceError>(42) }, config).await;

    assert_eq!(result.expect("retry should succeed"), 42);
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

    result.unwrap_err();
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

    assert_eq!(result.expect("retry should eventually succeed"), 42);
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

    assert_eq!(result.expect("backoff retry should succeed"), "success");
}

#[tokio::test]
async fn test_retry_operation_with_backoff_fails() {
    let result = retry_operation_with_backoff(
        || async { Err::<String, AgentServiceError>(AgentServiceError::Timeout(100)) },
        2,
        Duration::from_millis(1),
    )
    .await;

    result.unwrap_err();
}

#[tokio::test]
async fn test_execute_with_timeout_succeeds() {
    let result = execute_with_timeout(Duration::from_secs(1), async {
        Ok::<i32, AgentServiceError>(42)
    })
    .await;

    assert_eq!(result.expect("timeout should not trigger"), 42);
}

#[tokio::test]
async fn test_execute_with_timeout_times_out() {
    let result = execute_with_timeout(Duration::from_millis(1), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<i32, AgentServiceError>(42)
    })
    .await;

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

    match result.unwrap_err() {
        AgentServiceError::Database(msg) => assert_eq!(msg, "db error"),
        _ => panic!("Expected Database error"),
    }
}

#[tokio::test]
async fn test_execute_with_custom_timeout_connect() {
    let config = TimeoutConfiguration {
        default: Duration::from_secs(30),
        connect: Duration::from_secs(1),
        read: Duration::from_secs(30),
        write: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Connect, async { Ok::<i32, AgentServiceError>(1) }).await;

    result.expect("connect timeout should succeed");
}

#[tokio::test]
async fn test_execute_with_custom_timeout_read() {
    let config = TimeoutConfiguration {
        default: Duration::from_secs(30),
        connect: Duration::from_secs(10),
        read: Duration::from_secs(1),
        write: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Read, async { Ok::<i32, AgentServiceError>(2) }).await;

    result.expect("read timeout should succeed");
}

#[tokio::test]
async fn test_execute_with_custom_timeout_write() {
    let config = TimeoutConfiguration {
        default: Duration::from_secs(30),
        connect: Duration::from_secs(10),
        read: Duration::from_secs(30),
        write: Duration::from_secs(1),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Write, async { Ok::<i32, AgentServiceError>(3) }).await;

    result.expect("write timeout should succeed");
}

#[tokio::test]
async fn test_execute_with_custom_timeout_default() {
    let config = TimeoutConfiguration {
        default: Duration::from_secs(1),
        connect: Duration::from_secs(10),
        read: Duration::from_secs(30),
        write: Duration::from_secs(30),
    };

    let result =
        execute_with_custom_timeout(config, TimeoutType::Default, async { Ok::<i32, AgentServiceError>(4) }).await;

    result.expect("default timeout should succeed");
}
