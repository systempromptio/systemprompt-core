//! End-to-end tests for `ResilienceGuard`.

use std::io::{Error, ErrorKind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use systemprompt_database::resilience::classify::Outcome;
use systemprompt_database::resilience::config::{
    BreakerConfig, BulkheadConfig, ResilienceConfig, RetryConfig,
};
use systemprompt_database::resilience::error::ResilienceError;
use systemprompt_database::resilience::guard::ResilienceGuard;

fn classify(err: &Error) -> Outcome {
    if err.kind() == ErrorKind::TimedOut {
        Outcome::Transient { retry_after: None }
    } else {
        Outcome::Permanent
    }
}

fn config() -> ResilienceConfig {
    ResilienceConfig {
        request_timeout: Duration::from_millis(100),
        stream_idle_timeout: Duration::from_secs(1),
        retry: RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            jitter: false,
        },
        breaker: BreakerConfig {
            failure_threshold: 2,
            open_cooldown: Duration::from_millis(50),
            half_open_max_probes: 1,
        },
        bulkhead: BulkheadConfig { max_concurrent: 4 },
    }
}

#[tokio::test]
async fn retries_a_transient_failure_then_succeeds() {
    let guard = ResilienceGuard::new("dep", config());
    let attempts = AtomicU32::new(0);

    let result: Result<u32, ResilienceError<Error>> = guard
        .execute(classify, || async {
            let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 2 {
                Err(Error::new(ErrorKind::TimedOut, "transient"))
            } else {
                Ok(n)
            }
        })
        .await;

    assert_eq!(result.unwrap(), 2);
}

#[tokio::test]
async fn permanent_failure_surfaces_without_retry() {
    let guard = ResilienceGuard::new("dep", config());
    let attempts = AtomicU32::new(0);

    let result: Result<(), ResilienceError<Error>> = guard
        .execute(classify, || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(Error::new(ErrorKind::PermissionDenied, "permanent"))
        })
        .await;

    assert!(matches!(result, Err(ResilienceError::Inner(_))));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn breaker_opens_and_fast_fails_after_repeated_failures() {
    let guard = ResilienceGuard::new("dep", config());

    for _ in 0..2 {
        let result: Result<(), ResilienceError<Error>> = guard
            .execute(classify, || async {
                Err(Error::new(ErrorKind::TimedOut, "transient"))
            })
            .await;
        assert!(result.is_err());
    }

    let result: Result<(), ResilienceError<Error>> =
        guard.execute(classify, || async { Ok(()) }).await;
    match result {
        Err(ResilienceError::CircuitOpen { .. }) => {}
        other => panic!("expected CircuitOpen, got {other:?}"),
    }
}

#[tokio::test]
async fn an_attempt_exceeding_the_timeout_surfaces_as_timeout() {
    let guard = ResilienceGuard::new("dep", config());

    let result: Result<(), ResilienceError<Error>> = guard
        .execute(classify, || async {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Ok(())
        })
        .await;

    match result {
        Err(ResilienceError::Timeout { .. }) => {}
        other => panic!("expected Timeout, got {other:?}"),
    }
}
