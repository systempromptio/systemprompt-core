//! Initial-connect retry policy for `PostgresProvider`.
//!
//! Wraps the first `PgPool` connect in a bounded exponential backoff so
//! transient startup races (Postgres still booting, SSL handshake racing
//! the TCP listener) recover without surfacing as user-visible failures.
//! The retry loop intentionally targets a narrow set of error shapes so
//! permanent failures (auth, missing database, bad URL) fail fast. The
//! backoff itself runs on
//! [`systemprompt_storage::resilience::retry::retry_async`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::error::DatabaseResult;
use systemprompt_storage::resilience::classify::Outcome;
use systemprompt_storage::resilience::config::RetryConfig;
use systemprompt_storage::resilience::retry::retry_async;

const RETRY_DELAYS_MS: &[u64] = &[100, 200, 400, 800, 1600];
const MAX_ATTEMPTS: u32 = 5;
pub const DEFAULT_STATEMENT_CACHE_CAPACITY: usize = 100;

/// Schema-change stamp shared by every pool a [`crate::Database`] opens.
///
/// A connection opened before the last recorded schema change is closed on
/// its next acquire, so no prepared statement outlives the DDL under it.
#[derive(Debug, Clone)]
pub struct SchemaEpoch {
    inner: Arc<EpochState>,
}

#[derive(Debug)]
struct EpochState {
    clock: Instant,
    changed_at_nanos: AtomicU64,
}

impl Default for SchemaEpoch {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaEpoch {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EpochState {
                clock: Instant::now(),
                changed_at_nanos: AtomicU64::new(0),
            }),
        }
    }

    pub fn mark_changed(&self) {
        let now = nanos_since_clock(self.inner.clock.elapsed());
        self.inner.changed_at_nanos.store(now, Ordering::Release);
    }

    fn opened_after_change(&self, age: Duration) -> bool {
        let changed = self.inner.changed_at_nanos.load(Ordering::Acquire);
        if changed == 0 {
            return true;
        }
        let opened = self.inner.clock.elapsed().saturating_sub(age);
        nanos_since_clock(opened) >= changed
    }
}

fn nanos_since_clock(at: Duration) -> u64 {
    u64::try_from(at.as_nanos()).unwrap_or(u64::MAX).max(1)
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

pub const DEFAULT_SESSION_MAX_CONNECTIONS: u32 = 10;

impl PoolConfig {
    #[must_use]
    pub const fn for_session(self, max_connections: Option<u32>) -> Self {
        Self {
            max_connections: match max_connections {
                Some(max) => max,
                None => DEFAULT_SESSION_MAX_CONNECTIONS,
            },
            ..self
        }
    }
}

/// A node's declared Postgres connection budget (`database.connection_budget`)
/// and the rule that divides it.
///
/// For a node: a fifth of the budget, in whole [`CHILD_POOL_CONNECTIONS`]
/// shares, is reserved for the hosted children it spawns; a separate session
/// pool takes a tenth (at least [`MIN_SESSION_CONNECTIONS`]: the
/// notification hub's `LISTEN`, the instance claim and one lock or
/// migration session); the rest is the main pool, split evenly between the
/// read and write pools when `database_write_url` names a second one. An
/// explicit `database.pool.max_connections` or `session_max_connections`
/// is kept and checked against the budget instead. A hosted child takes one
/// fixed share and opens no hub session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionBudget {
    total: u32,
}

pub const CHILD_POOL_CONNECTIONS: u32 = 2;
pub const MIN_SESSION_CONNECTIONS: u32 = 3;
const MIN_MAIN_POOL_CONNECTIONS: u32 = 2;
const CHILD_RESERVE_DIVISOR: u32 = 5;
const SESSION_DIVISOR: u32 = 10;

/// Whose pools a budget is divided for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSeat {
    Node,
    Child,
}

/// Which separate pools the node's secrets open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolShape {
    pub separate_write: bool,
    pub separate_session: bool,
}

/// Pool sizes an operator pinned in `database.pool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinnedPools {
    pub main: Option<u32>,
    pub session: Option<u32>,
}

/// How a budget was divided: `main` sizes each of the `main_pools` (the
/// primary pool, and the write pool when one is separate), `session` the
/// separate session pool, and `children` is the share reserved for hosted
/// children, not opened by the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSizes {
    pub main: u32,
    pub main_pools: u32,
    pub session: u32,
    pub children: u32,
}

impl PoolSizes {
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.main * self.main_pools + self.session + self.children
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BudgetError {
    #[error(
        "database.connection_budget {budget} is below the {required} connections this node's \
         pools, session pool and children need"
    )]
    TooSmall { budget: u32, required: u32 },
    #[error("database.pool sizes need {parts} connections, more than database.connection_budget {budget}")]
    PartsExceed { budget: u32, parts: u32 },
}

impl ConnectionBudget {
    #[must_use]
    pub const fn new(total: u32) -> Self {
        Self { total }
    }

    #[must_use]
    pub const fn total(self) -> u32 {
        self.total
    }

    pub fn divide(
        self,
        seat: PoolSeat,
        shape: PoolShape,
        pinned: PinnedPools,
    ) -> Result<PoolSizes, BudgetError> {
        let main_pools = if shape.separate_write { 2 } else { 1 };
        let sizes = match seat {
            PoolSeat::Child => PoolSizes {
                main: CHILD_POOL_CONNECTIONS / main_pools,
                main_pools,
                session: u32::from(shape.separate_session),
                children: 0,
            },
            PoolSeat::Node => self.divide_node(shape, pinned, main_pools)?,
        };
        if sizes.total() > self.total {
            return Err(BudgetError::PartsExceed {
                budget: self.total,
                parts: sizes.total(),
            });
        }
        Ok(sizes)
    }

    fn divide_node(
        self,
        shape: PoolShape,
        pinned: PinnedPools,
        main_pools: u32,
    ) -> Result<PoolSizes, BudgetError> {
        let children = self.total / CHILD_RESERVE_DIVISOR / CHILD_POOL_CONNECTIONS
            * CHILD_POOL_CONNECTIONS;
        let session = if shape.separate_session {
            pinned
                .session
                .unwrap_or_else(|| (self.total / SESSION_DIVISOR).max(MIN_SESSION_CONNECTIONS))
        } else {
            0
        };
        let required = children
            + if shape.separate_session { MIN_SESSION_CONNECTIONS } else { 0 }
            + MIN_MAIN_POOL_CONNECTIONS * main_pools;
        if self.total < required {
            return Err(BudgetError::TooSmall {
                budget: self.total,
                required,
            });
        }
        let main = pinned.main.unwrap_or_else(|| {
            self.total.saturating_sub(children + session) / main_pools
        });
        if main < MIN_MAIN_POOL_CONNECTIONS {
            return Err(BudgetError::TooSmall {
                budget: self.total,
                required,
            });
        }
        Ok(PoolSizes {
            main,
            main_pools,
            session,
            children,
        })
    }
}

#[must_use]
pub fn build_pool_options(cfg: &PoolConfig, epoch: &SchemaEpoch) -> PgPoolOptions {
    let epoch = epoch.clone();
    PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .max_lifetime(cfg.max_lifetime)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        // Why: a cached prepared statement fails with SQLSTATE 0A000 ("cached
        // plan must not change result type") once DDL changes the table under
        // it. In-process migrations call `SchemaEpoch::mark_changed`, and every
        // connection opened before that is closed on its next acquire instead
        // of being handed out with stale plans.
        .before_acquire(move |_conn, meta| {
            let fresh = epoch.opened_after_change(meta.age);
            Box::pin(async move { Ok(fresh) })
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
        // Why: a transaction left open by a crashed or stuck task holds its row
        // locks and stops autovacuum reclaiming the hot tables until the
        // connection is recycled (30 min). Postgres ends a session idle inside
        // a transaction past this bound; no request or job legitimately sits
        // idle mid-transaction that long.
        .options([
            ("client_min_messages", "warning"),
            ("idle_in_transaction_session_timeout", "600000"),
        ]);
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
        jitter: true,
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
