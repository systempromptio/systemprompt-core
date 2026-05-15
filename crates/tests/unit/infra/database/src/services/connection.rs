use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use sqlx::postgres::PgConnectOptions;
use systemprompt_database::services::postgres::connection::connect_with_retry_using;

fn opts() -> PgConnectOptions {
    PgConnectOptions::from_str("postgres://u:p@127.0.0.1:5432/x").expect("parse url")
}

fn refused() -> sqlx::Error {
    sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused))
}

fn protocol_error(msg: &str) -> sqlx::Error {
    sqlx::Error::Protocol(msg.to_string())
}

#[tokio::test]
async fn succeeds_on_first_attempt_without_retry() {
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move { Ok(7u8) }
    })
    .await;

    assert_eq!(result.expect("ok"), 7u8);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_connection_refused_then_succeeds() {
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            if attempt < 3 {
                Err(refused())
            } else {
                Ok(42u8)
            }
        }
    })
    .await;

    assert_eq!(result.expect("ok"), 42u8);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn retries_ssl_request_error_message() {
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            if attempt < 2 {
                Err(protocol_error("unexpected response from SSLRequest"))
            } else {
                Ok(1u8)
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retries_starting_up_message() {
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        let attempt = calls.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            if attempt < 2 {
                Err(protocol_error("the database system is starting up"))
            } else {
                Ok(1u8)
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn gives_up_after_five_attempts() {
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move { Err(refused()) }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn non_retryable_error_fails_immediately() {
    let errors = Mutex::new(vec![
        sqlx::Error::Protocol("password authentication failed for user".to_string()),
    ]);
    let calls = AtomicU32::new(0);
    let result = connect_with_retry_using::<u8, _, _>(opts(), 5, &[1, 1, 1, 1, 1], |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        let next = errors.lock().expect("lock").pop();
        async move {
            match next {
                Some(e) => Err(e),
                None => Ok(0u8),
            }
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
