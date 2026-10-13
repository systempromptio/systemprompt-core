//! How long one migration statement may run before the database cancels it.
//!
//! Migrations are awaited inside the runtime builder, before the HTTP
//! listener is bound, so a statement that does not finish is not a slow
//! boot — it is an instance that never opens its port, with one log line
//! written before it started and nothing after. A production instance spent
//! 27 minutes there on a 3,644-row `UPDATE`, and the only signal was silence.
//!
//! So every migration runs under a `statement_timeout` and a `lock_timeout`.
//! The default is deliberately generous: too tight a bound turns a legitimate
//! upgrade into a refusal on a customer's server, which is the failure this
//! is meant to prevent, not cause. A migration that declares
//! [`CostDirective`](systemprompt_extension::CostDirective) gets a bound
//! derived from what its author actually measured, which is the point of
//! measuring — a statement running an order of magnitude over its measurement
//! is not slow, it is wrong.
//!
//! The constants: `DEFAULT_STATEMENT_TIMEOUT` of five minutes is longer than
//! any migration this codebase ships and short enough that a hung boot is
//! reported within a deploy's patience. A declared measurement is multiplied
//! by `MEASURED_SAFETY_FACTOR` before becoming the bound, so a slower disk, a
//! colder cache or a larger table than the author's copy does not trip it; ten
//! times the measured cost is still a clear defect. `MIN_DERIVED_TIMEOUT` is
//! the floor under a derived bound, so a migration measured at 20 ms does not
//! end up with a 200 ms timeout that any unrelated hiccup would breach.
//! `TIMEOUT_ENV` overrides the default in seconds, and `0` disables the
//! statement timeout entirely — the escape hatch for a deliberate, attended
//! one-off migration on a large instance. It outranks a declared cost: an
//! operator setting it is attending a specific run and knows what the author
//! could not.
//!
//! A measurement is taken on the author's fixture, so a derived bound is
//! projected onto the live database before it is applied: the measured cost
//! grows linearly with how far the largest table the migration names
//! exceeds the declared `rows` (see `CostDirective::projected`). Without it a
//! migration measured on a thousand sessions cancelled itself on a customer
//! holding a hundred times that. `MAX_DERIVED_TIMEOUT` caps the projection:
//! past half an hour the boot is better refused, and attended with
//! `TIMEOUT_ENV`, than left silent.
//!
//! `lock_timeout` is much shorter than `statement_timeout` on purpose: waiting
//! on a lock means another session holds the table, and blocking the whole
//! boot behind it is never the right answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_extension::{Migration, cost};

use crate::services::DatabaseProvider;

pub(crate) const DEFAULT_STATEMENT_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) const MEASURED_SAFETY_FACTOR: u32 = 10;

pub(crate) const MIN_DERIVED_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const MAX_DERIVED_TIMEOUT: Duration = Duration::from_mins(30);

pub(crate) const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) const TIMEOUT_ENV: &str = "SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS";

#[must_use]
pub(crate) fn statement_timeout(migration: &Migration, live_rows: u64) -> Option<Duration> {
    match env_timeout() {
        EnvTimeout::Unset => {},
        EnvTimeout::Disabled => return None,
        EnvTimeout::Bounded(timeout) => return Some(timeout),
    }
    if let Ok(Some(declared)) = cost::parse(migration.sql) {
        return Some(
            declared
                .projected(live_rows)
                .saturating_mul(MEASURED_SAFETY_FACTOR)
                .clamp(MIN_DERIVED_TIMEOUT, MAX_DERIVED_TIMEOUT),
        );
    }
    Some(DEFAULT_STATEMENT_TIMEOUT)
}

/// The largest row estimate among the tables `migration` names, or `0` when
/// it declares no cost (nothing to project) or names no populated table.
// Why: every table, not a fixed hot list — a downstream extension's busiest
// table is unknown to core, and a backfill over it timed out a customer's
// upgrade. `reltuples` is the planner's estimate (-1 before the first
// ANALYZE), read instead of `count(*)` so sizing a table never scans it.
pub(crate) async fn live_rows(db: &dyn DatabaseProvider, migration: &Migration) -> u64 {
    if !matches!(cost::parse(migration.sql), Ok(Some(_))) {
        return 0;
    }
    let rows = db
        .fetch_all(
            &"SELECT c.relname::text AS name, c.reltuples::bigint AS estimate FROM pg_class c \
              JOIN pg_namespace n ON n.oid = c.relnamespace \
              WHERE c.relkind IN ('r', 'p') AND n.nspname = current_schema() AND c.reltuples > 0",
            &[],
        )
        .await
        .inspect_err(|error| {
            tracing::warn!(%error, "Table size estimates unavailable; migration bound not projected");
        })
        .unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let name = row.get("name")?.as_str()?;
            let estimate = row.get("estimate")?.as_i64()?;
            if !names_table(migration.sql, name) {
                return None;
            }
            u64::try_from(estimate).ok()
        })
        .max()
        .unwrap_or(0)
}

fn names_table(sql: &str, table: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    sql.match_indices(table).any(|(at, _)| {
        let before = sql[..at].chars().next_back();
        let after = sql[at + table.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

enum EnvTimeout {
    Unset,
    Disabled,
    Bounded(Duration),
}

fn env_timeout() -> EnvTimeout {
    let Ok(raw) = std::env::var(TIMEOUT_ENV) else {
        return EnvTimeout::Unset;
    };
    match raw.trim().parse::<u64>() {
        Ok(0) => EnvTimeout::Disabled,
        Ok(secs) => EnvTimeout::Bounded(Duration::from_secs(secs)),
        // Why: a typo in the override must not silently remove the bound.
        Err(_) => EnvTimeout::Bounded(DEFAULT_STATEMENT_TIMEOUT),
    }
}

#[must_use]
pub(crate) fn timeout_statements(timeout: Option<Duration>, local: bool) -> Vec<String> {
    // Why: `SET LOCAL` is correct inside a transaction and a no-op outside
    // one, so the untransactioned path passes `local = false`.
    let scope = if local { "LOCAL " } else { "" };
    let statement = timeout.map_or_else(|| "0".to_owned(), |d| format!("{}", d.as_millis()));
    vec![
        format!("SET {scope}statement_timeout = {statement}"),
        format!("SET {scope}lock_timeout = {}", LOCK_TIMEOUT.as_millis()),
    ]
}
