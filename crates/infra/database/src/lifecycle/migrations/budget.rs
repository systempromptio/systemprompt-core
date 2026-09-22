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
//! `lock_timeout` is much shorter than `statement_timeout` on purpose: waiting
//! on a lock means another session holds the table, and blocking the whole
//! boot behind it is never the right answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_extension::{Migration, cost};

pub(crate) const DEFAULT_STATEMENT_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) const MEASURED_SAFETY_FACTOR: u32 = 10;

pub(crate) const MIN_DERIVED_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) const TIMEOUT_ENV: &str = "SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS";

#[must_use]
pub(crate) fn statement_timeout(migration: &Migration) -> Option<Duration> {
    match env_timeout() {
        EnvTimeout::Unset => {},
        EnvTimeout::Disabled => return None,
        EnvTimeout::Bounded(timeout) => return Some(timeout),
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
