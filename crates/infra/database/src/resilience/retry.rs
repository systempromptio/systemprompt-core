//! Bounded exponential-backoff retry with full jitter.

use std::future::Future;
use std::time::Duration;

use super::classify::Outcome;
use super::config::RetryConfig;

pub async fn retry_async<T, E, F, Fut>(
    cfg: &RetryConfig,
    key: &str,
    classify: impl Fn(&E) -> Outcome,
    op: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let err = match op().await {
            Ok(value) => return Ok(value),
            Err(err) => err,
        };

        let retry_after = match classify(&err) {
            Outcome::Transient { retry_after } => retry_after,
            Outcome::Success | Outcome::Permanent => return Err(err),
        };
        if attempt >= cfg.max_attempts {
            return Err(err);
        }

        let delay = next_delay(cfg, attempt, retry_after);
        tracing::warn!(
            key,
            attempt,
            max_attempts = cfg.max_attempts,
            next_delay_ms = delay.as_millis() as u64,
            error = %err,
            "retrying transient failure",
        );
        tokio::time::sleep(delay).await;
    }
}

fn next_delay(cfg: &RetryConfig, attempt: u32, retry_after: Option<Duration>) -> Duration {
    let shift = attempt.saturating_sub(1).min(16);
    let factor = 1u32 << shift;
    let mut delay = cfg.base_delay.saturating_mul(factor).min(cfg.max_delay);

    if cfg.jitter {
        let millis = delay.as_millis() as u64;
        if millis > 0 {
            delay = Duration::from_millis(rand::random_range(0..=millis));
        }
    }

    retry_after.map_or(delay, |hint| delay.max(hint))
}
