//! Store, don't compute: the data paradigm and the queue that carries it.
//!
//! Every analytics-shaped read in the system follows one rule. The answer a
//! page, report or policy needs was known once — when the row it summarises
//! was written — and is stored then, so no reader ever re-derives it from
//! the raw history. The cost of a read is then the size of the answer, never
//! the size of the ledger behind it.
//!
//! The vocabulary, from raw to read:
//!
//! 1. **Ledger** — the append-only raw tables the gateway, the hook plane, the
//!    tool plane and the governance spine write (`ai_requests`,
//!    `ai_request_payloads`, `ai_request_messages`, `mcp_tool_executions`,
//!    `plugin_usage_events`, `governance_decisions`, `logs`, …). Read by
//!    exactly two things: projectors, and single-entity detail queries (`WHERE
//!    pk = $1`, no aggregate).
//! 2. **Stamped fact** — a pure function of the row and what is in hand when it
//!    is written, stored as a column in the same transaction by the table's
//!    owner (`has_offered_tools`, `is_builtin`, `skill_ref`).
//! 3. **Counter** — an additive aggregate the writer keeps current through a
//!    `FOR EACH STATEMENT … REFERENCING NEW TABLE` trigger (never a row
//!    trigger: a row trigger's cost is the fan-out that turned a 3,644-row
//!    update into 27 minutes).
//! 4. **Projection** — one denormalised row per entity (`<entity>_facts`),
//!    maintained by `refresh_<projection>(keys TEXT[])` and drained from the
//!    generic queue in this module.
//! 5. **Rollup** — `<subject>_rollup_hourly(bucket, dims…)` of additive
//!    measures, upserted at write time. Hourly, because analytics windows are
//!    arbitrary timestamps; percentiles, distinct counts and top-N substrings
//!    are not additive and keep reading raw rows for their window.
//!
//! **Every fact has four parts, all in the base schema**: the structure, the
//! projector or stamp, the statement trigger (stamp directly when the
//! transition table alone determines the value, else queue the keys) and the
//! drainer job. An upgrade migration is the same DDL text plus one
//! `INSERT INTO projection_backlog SELECT '<projection>', <key> FROM …`.
//! Nothing else: a fresh install stamps migrations without running them, so
//! anything a migration computes exists only on upgraded databases.
//!
//! **NULL means pending.** A reader never falls back to the ledger expression
//! (`COALESCE(fact, <ledger expr>)`); it shows the fact as pending and
//! [`depth`] says how many are. Two static gates keep this true: the
//! ledger-reads gate over repository SQL, and `no_backfill_after_floor` over
//! migrations, built on [`crate::backfill_statements`].
//!
//! ## The queue
//!
//! `projection_backlog(projection, key, queued_at, attempts)` is one table
//! for every projection. Writers queue keys ([`queue`] or the SQL
//! `queue_projection_backlog`); a drainer job calls [`drain`] with the
//! projection's refresh, which takes a batch with `FOR UPDATE SKIP LOCKED`,
//! refreshes it and deletes the rows in one transaction. A failed batch is
//! rolled back and re-queued one attempt dearer; a key past its retry budget
//! stays queued for an operator and never blocks the rest. A statement that
//! keeps a `refresh_*(keys TEXT[])` SQL function can use [`drain_with_sql`]
//! and name it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use sqlx::{PgConnection, PgExecutor, PgPool};
use systemprompt_traits::RepositoryError;
use tracing::warn;

use crate::services::BoxFuture;

/// How a drainer tick went. `remaining` is the queue depth when it stopped,
/// so a job can report "partly drained" without another query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrainReport {
    pub keys: u64,
    pub batches: u32,
    pub remaining: i64,
    pub elapsed: Duration,
    pub requeued: u64,
}

/// A projection's refresh: given a connection inside the drain transaction
/// and the keys taken, rebuild them and return how many rows were written.
pub type RefreshFuture<'c> = BoxFuture<'c, Result<u64, RepositoryError>>;

pub async fn queue<'e, E>(
    executor: E,
    projection: &str,
    keys: &[String],
) -> Result<i64, RepositoryError>
where
    E: PgExecutor<'e>,
{
    let queued: i64 = sqlx::query_scalar("SELECT queue_projection_backlog($1, $2)")
        .bind(projection)
        .bind(keys)
        .fetch_one(executor)
        .await?;
    Ok(queued)
}

pub async fn depth<'e, E>(executor: E, projection: &str) -> Result<i64, RepositoryError>
where
    E: PgExecutor<'e>,
{
    let depth: i64 = sqlx::query_scalar("SELECT projection_backlog_depth($1)")
        .bind(projection)
        .fetch_one(executor)
        .await?;
    Ok(depth)
}

pub async fn depths<'e, E>(executor: E) -> Result<Vec<(String, i64)>, RepositoryError>
where
    E: PgExecutor<'e>,
{
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT projection, COUNT(*)::BIGINT FROM projection_backlog GROUP BY projection",
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn drain<F>(
    pool: &PgPool,
    projection: &str,
    batch: i32,
    budget: Duration,
    refresh: F,
) -> Result<DrainReport, RepositoryError>
where
    F: for<'c> Fn(&'c mut PgConnection, Vec<String>) -> RefreshFuture<'c> + Send + Sync,
{
    let started = Instant::now();
    let mut report = DrainReport::default();
    while started.elapsed() < budget {
        let mut tx = pool.begin().await?;
        let keys: Vec<String> = sqlx::query_scalar("SELECT take_projection_backlog($1, $2)")
            .bind(projection)
            .bind(batch)
            .fetch_one(&mut *tx)
            .await?;
        if keys.is_empty() {
            tx.rollback().await?;
            break;
        }
        let taken = u64::try_from(keys.len()).unwrap_or(u64::MAX);
        match refresh(&mut tx, keys.clone()).await {
            Ok(_) => {
                tx.commit().await?;
                report.keys = report.keys.saturating_add(taken);
                report.batches = report.batches.saturating_add(1);
            },
            Err(error) => {
                tx.rollback().await?;
                let requeued: i64 = sqlx::query_scalar("SELECT requeue_projection_backlog($1, $2)")
                    .bind(projection)
                    .bind(&keys)
                    .fetch_one(pool)
                    .await?;
                report.requeued = u64::try_from(requeued).unwrap_or(0);
                warn!(
                    projection,
                    keys = keys.len(),
                    error = %error,
                    "projection refresh failed; batch re-queued one attempt dearer"
                );
                break;
            },
        }
    }
    report.remaining = depth(pool, projection).await?;
    report.elapsed = started.elapsed();
    Ok(report)
}

pub async fn drain_with_sql(
    pool: &PgPool,
    projection: &str,
    batch: i32,
    budget: Duration,
    projector: &str,
) -> Result<DrainReport, RepositoryError> {
    let statement = projector_statement(projector)?;
    let statement = statement.as_str();
    drain(pool, projection, batch, budget, move |conn, keys| {
        let statement = statement.to_owned();
        Box::pin(async move {
            // Why: Postgres — the only dynamic part is the projector's name,
            // checked as a bare identifier by `projector_statement`; the keys
            // are a bind parameter.
            let written: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(statement.as_str()))
                .bind(&keys)
                .fetch_one(conn)
                .await?;
            Ok(u64::try_from(written).unwrap_or(0))
        })
    })
    .await
}

fn projector_statement(projector: &str) -> Result<String, RepositoryError> {
    let mut chars = projector.chars();
    let leads = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !(leads && rest) {
        return Err(RepositoryError::InvalidArgument {
            field: "projector",
            reason: format!("{projector:?} is not a bare lower-case identifier"),
        });
    }
    Ok(format!("SELECT {projector}($1)"))
}
