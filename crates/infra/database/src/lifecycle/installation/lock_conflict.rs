//! Which schema-phase failures are a transient lock conflict with live
//! traffic, so the phase is rolled back and retried rather than failing the
//! install.
//!
//! The decision reads the Postgres SQLSTATE the error chain carries:
//! `55P03` `lock_not_available` (the phase's `lock_timeout`), `40P01`
//! `deadlock_detected` (re-creating a trigger while writers hold row locks on
//! the tables it touches) and `40001` `serialization_failure`. Any other
//! SQLSTATE is not retried, whatever its message says, and neither is a chain
//! that carries no SQLSTATE: every error Postgres raises carries one, so a
//! chain without it did not come from the server and is no lock conflict.
//!
//! A catalog scan never meets an object dropped mid-scan by an install:
//! every schema install holds the bootstrap lock exclusively, and a scan
//! outside an install holds it shared (`BootstrapLockGuard::acquire_shared`),
//! so the two never overlap. [`read_catalog`] reads the scan again only on
//! the same three SQLSTATEs, at most ten times within a second.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;
use std::future::Future;
use std::time::Duration;

use systemprompt_traits::RepositoryError;

pub const RETRYABLE_SQLSTATES: [&str; 3] = ["55P03", "40P01", "40001"];

#[must_use]
pub fn is_retryable_sqlstate(code: &str) -> bool {
    RETRYABLE_SQLSTATES.contains(&code)
}

#[must_use]
pub fn is_retryable_lock_conflict(error: &(dyn Error + 'static)) -> bool {
    sqlstate_in_chain(error).is_some_and(|code| is_retryable_sqlstate(&code))
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

const CATALOG_READ_ATTEMPTS: u32 = 10;

pub(crate) async fn read_catalog<T, E, F, Fut>(what: &'static str, mut read: F) -> Result<T, E>
where
    E: Error + 'static,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempt = 1;
    loop {
        match read().await {
            Err(error) if attempt < CATALOG_READ_ATTEMPTS && is_retryable_lock_conflict(&error) => {
                tracing::debug!(
                    attempt,
                    read = what,
                    error = %error,
                    "Catalog read hit a lock conflict; reading again",
                );
                tokio::time::sleep(Duration::from_millis(u64::from(attempt) * 20)).await;
                attempt += 1;
            },
            result => return result,
        }
    }
}
