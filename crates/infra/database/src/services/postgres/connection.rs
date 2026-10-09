//! Initial-connect retry policy for `PostgresProvider`.
//!
//! Wraps the first `PgPool` connect in a bounded exponential backoff so
//! transient startup races (Postgres still booting, SSL handshake racing
//! the TCP listener) recover without surfacing as user-visible failures.
//! The retry loop intentionally targets a narrow set of error shapes so
//! permanent failures (auth, missing database, bad URL) fail fast. The
//! backoff itself runs on [`crate::resilience::retry::retry_async`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::str::FromStr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::error::DatabaseResult;
use crate::resilience::classify::Outcome;
use crate::resilience::config::RetryConfig;
use crate::resilience::retry::retry_async;

const RETRY_DELAYS_MS: &[u64] = &[100, 200, 400, 800, 1600];
const MAX_ATTEMPTS: u32 = 5;
pub const DEFAULT_STATEMENT_CACHE_CAPACITY: usize = 100;

static POOL_CLOCK: OnceLock<Instant> = OnceLock::new();
static SCHEMA_CHANGED_AT_NANOS: AtomicU64 = AtomicU64::new(0);

fn pool_clock() -> Instant {
    *POOL_CLOCK.get_or_init(Instant::now)
}

fn nanos_since_clock(at: Duration) -> u64 {
    u64::try_from(at.as_nanos()).unwrap_or(u64::MAX).max(1)
}

pub fn mark_schema_changed() {
    let now = nanos_since_clock(pool_clock().elapsed());
    SCHEMA_CHANGED_AT_NANOS.store(now, Ordering::Release);
}

fn opened_after_schema_change(age: Duration) -> bool {
    let changed = SCHEMA_CHANGED_AT_NANOS.load(Ordering::Acquire);
    if changed == 0 {
        return true;
    }
    let opened = pool_clock().elapsed().saturating_sub(age);
    nanos_since_clock(opened) >= changed
}

/// Operator-tunable connection-pool sizing for a `PostgresProvider`.
///
/// Only the sizing/lifetime knobs an operator needs to fit the pool to their
/// Postgres `max_connections` and replica count are exposed; the connect, SSL
/// and retry behaviour is fixed.
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub statement_cache_capacity: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 50,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_mins(5),
            max_lifetime: Duration::from_mins(30),
            statement_cache_capacity: DEFAULT_STATEMENT_CACHE_CAPACITY,
        }
    }
}

#[must_use]
pub fn build_pool_options(cfg: &PoolConfig) -> PgPoolOptions {
    pool_clock();
    PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .max_lifetime(cfg.max_lifetime)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        // Why: a cached prepared statement fails with SQLSTATE 0A000 ("cached
        // plan must not change result type") once DDL changes the table under
        // it. In-process migrations call `mark_schema_changed`, and every
        // connection opened before that is closed on its next acquire instead
        // of being handed out with stale plans.
        .before_acquire(|_conn, meta| {
            Box::pin(async move { Ok(opened_after_schema_change(meta.age)) })
        })
}

pub fn connect_options(database_url: &str) -> DatabaseResult<PgConnectOptions> {
    let options = PgConnectOptions::from_str(database_url)?
        .application_name("systemprompt")
        // Why: sqlx 0.9 `PgConnection::get_or_prepare` Parses every persistent
        // query as a NAMED statement and only sends Close when the cache is
        // enabled and evicts, so capacity 0 never deallocates them: each
        // backend grew without bound until Postgres OOMed. A bounded cache
        // closes on eviction; stale plans after in-process DDL are handled by
        // `mark_schema_changed` in `build_pool_options`.
        .statement_cache_capacity(DEFAULT_STATEMENT_CACHE_CAPACITY)
        .options([("client_min_messages", "warning")]);
    Ok(options)
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
    if let sqlx::Error::Io(io_err) = err
        && io_err.kind() == std::io::ErrorKind::ConnectionRefused
    {
        return true;
    }
    let msg = err.to_string();
    msg.contains("unexpected response from SSLRequest") || msg.contains("starting up")
}
