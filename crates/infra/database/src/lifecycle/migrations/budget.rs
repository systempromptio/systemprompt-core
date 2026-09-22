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
//! `lock_timeout` is much shorter than `statement_timeout` on purpose: waiting
//! on a lock means another session holds the table, and blocking the whole
//! boot behind it is never the right answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_extension::{Migration, cost};

/// Applied to a migration that measured nothing. Five minutes is longer than
/// any migration this codebase ships and short enough that a hung boot is
/// reported within a deploy's patience.
pub(crate) const DEFAULT_STATEMENT_TIMEOUT: Duration = Duration::from_secs(300);

/// A declared measurement is multiplied by this before becoming the bound, so
/// a slower disk, a colder cache or a larger table than the author's copy does
/// not trip it. Ten times the measured cost is still a clear defect.
pub(crate) const MEASURED_SAFETY_FACTOR: u32 = 10;

/// The floor under a derived bound: a migration measured at 20 ms must not end
/// up with a 200 ms timeout, which any unrelated hiccup would breach.
pub(crate) const MIN_DERIVED_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Environment override for the default, in seconds. `0` disables the
/// statement timeout entirely — the escape hatch for a deliberate, attended
/// one-off migration on a large instance.
pub(crate) const TIMEOUT_ENV: &str = "SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS";

/// The bound for one migration: its declared measurement if it has one, else
/// the default (or its environment override).
// Why the override outranks the declaration: an operator who sets the
// variable is attending a specific run on a specific instance, and knows
// something the migration's author could not — that this table is ten times
// the size it was measured against, or that the run is expected to take an
// hour and is being watched. A declared cost that still clamped that run
// would make the escape hatch no escape at all.
#[must_use]
pub(crate) fn statement_timeout(migration: &Migration) -> Option<Duration> {
    if let Some(override_timeout) = env_timeout() {
        return override_timeout;
    }
    if let Ok(Some(declared)) = cost::parse(migration.sql) {
        return Some(
            declared
                .measured
                .saturating_mul(MEASURED_SAFETY_FACTOR)
                .max(MIN_DERIVED_TIMEOUT),
        );
    }
    Some(DEFAULT_STATEMENT_TIMEOUT)
}

/// `None` when unset, `Some(None)` when set to 0 (bound disabled).
fn env_timeout() -> Option<Option<Duration>> {
    let raw = std::env::var(TIMEOUT_ENV).ok()?;
    match raw.trim().parse::<u64>() {
        Ok(0) => Some(None),
        Ok(secs) => Some(Some(Duration::from_secs(secs))),
        // Why: a typo in the override must not silently remove the bound.
        Err(_) => Some(Some(DEFAULT_STATEMENT_TIMEOUT)),
    }
}

/// The `SET` statements that apply a bound to the session about to run a
/// migration. `LOCAL` is correct inside a transaction and a no-op outside
/// one, so the untransactioned path passes `local = false`.
#[must_use]
pub(crate) fn timeout_statements(timeout: Option<Duration>, local: bool) -> Vec<String> {
    let scope = if local { "LOCAL " } else { "" };
    let statement = timeout.map_or_else(|| "0".to_owned(), |d| format!("{}", d.as_millis()));
    vec![
        format!("SET {scope}statement_timeout = {statement}"),
        format!("SET {scope}lock_timeout = {}", LOCK_TIMEOUT.as_millis()),
    ]
}
