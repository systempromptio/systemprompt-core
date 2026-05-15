//! Initial-connect retry policy for `PostgresProvider`.
//!
//! Wraps the first `PgPool` connect in a bounded exponential backoff so
//! transient startup races (Postgres still booting, SSL handshake racing
//! the TCP listener) recover without surfacing as user-visible failures.
//! The retry loop intentionally targets a narrow set of error shapes so
//! permanent failures (auth, missing database, bad URL) fail fast.

use std::future::Future;
use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::error::DatabaseResult;

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
    F: Fn(PgConnectOptions) -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    let started = Instant::now();
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match connector(connect_options.clone()).await {
            Ok(pool) => {
                if attempt > 1 {
                    tracing::info!(
                        attempts = attempt,
                        elapsed_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        "connected after {attempt} attempts"
                    );
                }
                return Ok(pool);
            },
            Err(err) => {
                let retryable = is_retryable(&err);
                if !retryable || attempt >= max_attempts {
                    return Err(err.into());
                }
                let delay_idx = usize::try_from(attempt.saturating_sub(1)).unwrap_or(usize::MAX);
                let delay = delays_ms.get(delay_idx).copied().unwrap_or(0);
                tracing::warn!(
                    attempt,
                    elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    next_delay_ms = delay,
                    error = %err,
                    "postgres connect failed, retrying"
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            },
        }
    }
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
