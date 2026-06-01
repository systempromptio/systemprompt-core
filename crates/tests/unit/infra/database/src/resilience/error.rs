//! Tests for `ResilienceError` variants.

use std::time::Duration;

use systemprompt_database::resilience::error::ResilienceError;

type SampleError = std::io::Error;

#[test]
fn circuit_open_display_contains_key() {
    let err: ResilienceError<SampleError> = ResilienceError::CircuitOpen {
        key: "my-service".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-service"), "display: {msg}");
    assert!(msg.contains("open"), "display: {msg}");
}

#[test]
fn bulkhead_full_display_contains_key_and_limit() {
    let err: ResilienceError<SampleError> = ResilienceError::BulkheadFull {
        key: "ai-provider".to_string(),
        limit: 8,
    };
    let msg = err.to_string();
    assert!(msg.contains("ai-provider"), "display: {msg}");
    assert!(msg.contains("8"), "display: {msg}");
}

#[test]
fn timeout_display_contains_duration() {
    let err: ResilienceError<SampleError> = ResilienceError::Timeout {
        after: Duration::from_secs(30),
    };
    let msg = err.to_string();
    assert!(msg.contains("30"), "display: {msg}");
}

#[test]
fn inner_wraps_caller_error() {
    let io_err = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
    let err = ResilienceError::Inner(io_err);
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

#[test]
fn circuit_open_debug() {
    let err: ResilienceError<SampleError> = ResilienceError::CircuitOpen {
        key: "dep".to_string(),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("CircuitOpen"));
}

#[test]
fn bulkhead_full_debug() {
    let err: ResilienceError<SampleError> = ResilienceError::BulkheadFull {
        key: "dep".to_string(),
        limit: 4,
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("BulkheadFull"));
}

#[test]
fn timeout_debug() {
    let err: ResilienceError<SampleError> = ResilienceError::Timeout {
        after: Duration::from_millis(100),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("Timeout"));
}

#[test]
fn from_io_error_produces_inner() {
    let io_err = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
    let resilience_err: ResilienceError<std::io::Error> = ResilienceError::Inner(io_err);
    assert!(matches!(resilience_err, ResilienceError::Inner(_)));
}
