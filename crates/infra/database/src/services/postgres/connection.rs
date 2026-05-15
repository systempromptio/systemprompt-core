//! Initial-connect retry policy for `PostgresProvider`.
//!
//! Wraps the first `PgPool` connect in a bounded exponential backoff so
//! transient startup races (Postgres still booting, SSL handshake racing
//! the TCP listener) recover without surfacing as user-visible failures.
//! The retry loop intentionally targets a narrow set of error shapes so
//! permanent failures (auth, missing database, bad URL) fail fast. The
//! backoff itself runs on [`crate::resilience::retry::retry_async`].

use std::future::Future;
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::error::DatabaseResult;
use crate::resilience::classify::Outcome;
use crate::resilience::config::RetryConfig;
use crate::resilience::retry::retry_async;

const RETRY_DELAYS_MS: &[u64] = &[100, 200, 400, 800, 1600];
const MAX_ATTEMPTS: u32 = 5;

#[must_use]
pub fn build_pool_options() -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(50)
        .min_connections(0)
        .max_lifetime(Duration::from_secs(1800))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
}

pub async fn connect_with_retry(
    options: PgPoolOptions,
    connect_options: PgConnectOptions,
) -> DatabaseResult<PgPool> {
    let connector = |opts: PgConnectOptions| {
        let options = options.clone();
        async move { options.connect_with(opts).await }
    };
    connect_with_retry_using(connect_options, MAX_ATTEMPTS, RETRY_DELAYS_MS, connector).await
}

pub async fn connect_with_retry_using<T, F, Fut>(
    connect_options: PgConnectOptions,
    max_attempts: u32,
    delays_ms: &[u64],
    connector: F,
) -> DatabaseResult<T>
where
    T: Send,
    F: Fn(PgConnectOptions) -> Fut + Send + Sync,
    Fut: Future<Output = Result<T, sqlx::Error>> + Send,
{
    let cfg = RetryConfig {
        max_attempts,
        base_delay: Duration::from_millis(delays_ms.first().copied().unwrap_or(100)),
        max_delay: Duration::from_millis(delays_ms.iter().copied().max().unwrap_or(1600)),
        jitter: false,
    };
    let classify = |err: &sqlx::Error| {
        if is_retryable(err) {
            Outcome::Transient { retry_after: None }
        } else {
            Outcome::Permanent
        }
    };
    retry_async(&cfg, "postgres-connect", classify, || {
        connector(connect_options.clone())
    })
    .await
    .map_err(Into::into)
}

fn is_retryable(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Io(io_err) = err {
        if io_err.kind() == std::io::ErrorKind::ConnectionRefused {
            return true;
        }
    }
    let msg = err.to_string();
    msg.contains("unexpected response from SSLRequest") || msg.contains("starting up")
}
