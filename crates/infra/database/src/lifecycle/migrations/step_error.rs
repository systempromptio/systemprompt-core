//! Typed failure of one step inside a migration run: parsing its SQL,
//! suspending or restoring row triggers, bounding its timeouts, and executing
//! its statements and tracking write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use crate::error::RepositoryError;

#[derive(Debug, Error)]
pub(super) enum MigrationStepError {
    #[error("Failed to parse migration {version} ({name}) for {purpose}: {source}")]
    Parse {
        version: u32,
        name: String,
        purpose: &'static str,
        #[source]
        source: pg_query::Error,
    },

    #[error("Failed to list triggers on {table}: {source}")]
    ListTriggers {
        table: String,
        #[source]
        source: RepositoryError,
    },

    #[error("Failed to {action} trigger {trigger} on {table}: {source}")]
    Trigger {
        action: &'static str,
        trigger: String,
        table: String,
        #[source]
        source: RepositoryError,
    },

    #[error("Failed to bound migration {version} ({name}) with `{setting}`: {source}")]
    Bound {
        version: u32,
        name: String,
        setting: String,
        #[source]
        source: RepositoryError,
    },

    #[error(
        "Migration {version} ({name}) statement {n}/{total} failed: {source}\nSQL:\n{statement}"
    )]
    Statement {
        version: u32,
        name: String,
        n: usize,
        total: usize,
        statement: String,
        #[source]
        source: RepositoryError,
    },

    #[error("Migration {version} ({name}) tracking write failed: {source}")]
    Tracking {
        version: u32,
        name: String,
        #[source]
        source: RepositoryError,
    },
}
