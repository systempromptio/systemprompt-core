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
    #[error("Failed to parse migration {version} ({name}) for {purpose}")]
    Parse {
        version: u32,
        name: String,
        purpose: &'static str,
        #[source]
        source: pg_query::Error,
    },

    #[error("Failed to list triggers on {table}")]
    ListTriggers {
        table: String,
        #[source]
        source: Box<RepositoryError>,
    },

    #[error("Failed to {action} trigger {trigger} on {table}")]
    Trigger {
        action: &'static str,
        trigger: String,
        table: String,
        #[source]
        source: Box<RepositoryError>,
    },

    #[error("Failed to bound migration {version} ({name}) with `{setting}`")]
    Bound {
        version: u32,
        name: String,
        setting: String,
        #[source]
        source: Box<RepositoryError>,
    },

    #[error("Migration {version} ({name}) statement {n}/{total} failed\nSQL:\n{statement}")]
    Statement {
        version: u32,
        name: String,
        n: usize,
        total: usize,
        statement: String,
        #[source]
        source: Box<RepositoryError>,
    },

    #[error("Migration {version} ({name}) tracking write failed")]
    Tracking {
        version: u32,
        name: String,
        #[source]
        source: Box<RepositoryError>,
    },
}
