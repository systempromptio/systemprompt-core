//! Tests for `retry_async`.

use std::io::{Error, ErrorKind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use systemprompt_database::resilience::classify::Outcome;
use systemprompt_database::resilience::config::RetryConfig;
use systemprompt_database::resilience::retry::retry_async;

fn transient() -> Error {
    Error::new(ErrorKind::TimedOut, "transient")
}

fn permanent() -> Error {
    Error::new(ErrorKind::PermissionDenied, "permanent")
}

fn classify(err: &Error) -> Outcome {
    if err.kind() == ErrorKind::TimedOut {
        Outcome::Transient { retry_after: None }
    } else {
        Outcome::Permanent
    }
}

fn fast_config(max_attempts: u32) -> RetryConfig {
    RetryConfig {
        max_attempts,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
        jitter: false,
    }
}

#[tokio::test]
async fn retries_transient_until_success() {
    let attempts = AtomicU32::new(0);
    let result: Result<u32, Error> = retry_async(&fast_config(5), "dep", classify, || async {
        let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if n < 3 { Err(transient()) } else { Ok(n) }
    })
    .await;

    assert_eq!(result.unwrap(), 3);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn permanent_failure_is_not_retried() {
    let attempts = AtomicU32::new(0);
    let result: Result<(), Error> = retry_async(&fast_config(5), "dep", classify, || async {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err(permanent())
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stops_at_max_attempts() {
    let attempts = AtomicU32::new(0);
    let result: Result<(), Error> = retry_async(&fast_config(3), "dep", classify, || async {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err(transient())
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn single_attempt_disables_retry() {
    let attempts = AtomicU32::new(0);
    let result: Result<(), Error> = retry_async(&fast_config(1), "dep", classify, || async {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err(transient())
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
