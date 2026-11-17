//! Re-running a documents transaction that Postgres aborted as one side of a
//! deadlock (`40P01`) or a serialization conflict (`40001`).
//!
//! A document write holds `ROW EXCLUSIVE` on one documents table and asks for
//! another, so any session that locks those tables in the opposite order (DDL
//! that creates a missing index, an operator's `LOCK TABLE`, another writer)
//! can close a cycle with it. Postgres breaks the cycle by aborting one side,
//! and documents both codes as "retry the transaction". The aborted attempt was
//! rolled back and wrote nothing, so the whole transaction runs again from its
//! first statement. The decision reads the SQLSTATE and nothing else.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::Duration;

use systemprompt_database::resilience::{Outcome, RetryConfig, retry_async};
use systemprompt_traits::RepositoryError;

use super::blob_store::BlobStoreError;
use super::error::DocumentsError;

const RETRY: RetryConfig = RetryConfig {
    max_attempts: 5,
    base_delay: Duration::from_millis(25),
    max_delay: Duration::from_millis(400),
    jitter: true,
};

/// An error that may be the victim side of a deadlock or serialization
/// conflict, so the transaction that raised it may be run again.
pub trait TransientConflict {
    fn is_transient_conflict(&self) -> bool;
}

impl TransientConflict for RepositoryError {
    fn is_transient_conflict(&self) -> bool {
        self.is_serialization_failure()
    }
}

impl TransientConflict for DocumentsError {
    fn is_transient_conflict(&self) -> bool {
        matches!(self, Self::Repository(error) if error.is_serialization_failure())
    }
}

impl TransientConflict for BlobStoreError {
    fn is_transient_conflict(&self) -> bool {
        matches!(self, Self::Repository(error) if error.is_serialization_failure())
    }
}

pub async fn retry_conflicts<T, E, F, Fut>(transaction: F) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: TransientConflict + std::fmt::Display,
{
    let classify = |error: &E| {
        if error.is_transient_conflict() {
            Outcome::Transient { retry_after: None }
        } else {
            Outcome::Permanent
        }
    };
    retry_async(&RETRY, "documents transaction", classify, transaction).await
}
