//! Which schema-phase failures are a transient lock conflict with live
//! traffic, so the phase is rolled back and retried rather than failing the
//! install.
//!
//! The decision reads the Postgres SQLSTATE wherever the error chain carries
//! one: `55P03` `lock_not_available` (the phase's `lock_timeout`), `40P01`
//! `deadlock_detected` (re-creating a trigger while writers hold row locks on
//! the tables it touches) and `40001` `serialization_failure`. Any other
//! SQLSTATE is not retried, whatever its message says. Only a chain that
//! carries no SQLSTATE at all falls back to the server's message text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;

use systemprompt_traits::RepositoryError;

pub const RETRYABLE_SQLSTATES: [&str; 3] = ["55P03", "40P01", "40001"];

const RETRYABLE_MESSAGES: [&str; 3] = [
    "lock timeout",
    "deadlock detected",
    "could not serialize access",
];

#[must_use]
pub fn is_retryable_sqlstate(code: &str) -> bool {
    RETRYABLE_SQLSTATES.contains(&code)
}

#[must_use]
pub fn is_retryable_lock_conflict(error: &(dyn Error + 'static)) -> bool {
    sqlstate_in_chain(error).map_or_else(
        || message_in_chain(error),
        |code| is_retryable_sqlstate(&code),
    )
}

fn sqlstate_in_chain(error: &(dyn Error + 'static)) -> Option<String> {
    let mut source: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(current) = source {
        if let Some(RepositoryError::Database {
            sqlstate: Some(code),
            ..
        }) = current.downcast_ref::<RepositoryError>()
        {
            return Some(code.clone());
        }
        if let Some(code) = current
            .downcast_ref::<sqlx::Error>()
            .and_then(sqlx::Error::as_database_error)
            .and_then(sqlx::error::DatabaseError::code)
        {
            return Some(code.into_owned());
        }
        source = current.source();
    }
    None
}

fn message_in_chain(error: &(dyn Error + 'static)) -> bool {
    let mut source: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(current) = source {
        let text = current.to_string();
        if RETRYABLE_MESSAGES
            .iter()
            .any(|needle| text.contains(needle))
        {
            return true;
        }
        source = current.source();
    }
    false
}
