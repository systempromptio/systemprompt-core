use crate::services::shared::error::{AgentServiceError, Result};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Debug, Clone, Copy)]
pub struct RetryConfiguration {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: u32,
}

impl Default for RetryConfiguration {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            exponential_base: 2,
        }
    }
}

pub async fn retry_operation<F, Fut, T>(operation: F, config: RetryConfiguration) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut current_delay = config.initial_delay;

    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) if attempt == config.max_attempts => return Err(error),
            Err(_) => {
                sleep(current_delay).await;
                current_delay = calculate_next_delay(current_delay, &config);
            },
        }
    }

    Err(AgentServiceError::Configuration(
        "RetryConfiguration".to_string(),
        "Retry configuration resulted in no attempts".to_string(),
    ))
}

fn calculate_next_delay(current: Duration, config: &RetryConfiguration) -> Duration {
    let next = current.saturating_mul(config.exponential_base);
    if next > config.max_delay {
        config.max_delay
    } else {
        next
    }
}

pub async fn retry_operation_with_backoff<F, Fut, T>(
    operation: F,
    max_attempts: u32,
    initial_delay: Duration,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let config = RetryConfiguration {
        max_attempts,
        initial_delay,
        ..Default::default()
    };
    retry_operation(operation, config).await
}

pub async fn execute_with_timeout<F, T>(duration: Duration, operation: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    timeout(duration, operation)
        .await
        .unwrap_or_else(|_| Err(AgentServiceError::Timeout(duration.as_millis() as u64)))
}

#[derive(Debug, Clone, Copy)]
pub struct TimeoutConfiguration {
    pub default_timeout: Duration,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for TimeoutConfiguration {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
        }
    }
}

pub async fn execute_with_custom_timeout<F, T>(
    config: TimeoutConfiguration,
    timeout_type: TimeoutType,
    operation: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let duration = match timeout_type {
        TimeoutType::Connect => config.connect_timeout,
        TimeoutType::Read => config.read_timeout,
        TimeoutType::Write => config.write_timeout,
        TimeoutType::Default => config.default_timeout,
    };

    execute_with_timeout(duration, operation).await
}

#[derive(Debug, Clone, Copy)]
pub enum TimeoutType {
    Connect,
    Read,
    Write,
    Default,
}
