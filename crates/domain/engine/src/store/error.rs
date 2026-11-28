//! The store's error type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::store::id::IdError;

/// Every way a store operation can fail.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot create the directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot create the database file {path}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot open the database at {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("database schema version {found} is newer than this build supports ({supported})")]
    FutureSchema { found: i64, supported: i64 },
    #[error("migration to schema version {version} failed")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },
    #[error("sqlite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("stored JSON could not be encoded or decoded")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Id(#[from] IdError),
    #[error("{kind} {id} not found")]
    NotFound { kind: &'static str, id: String },
    #[error("part {id} does not hold text")]
    NotText { id: String },
    #[error("column {column} holds an unknown value {value:?}")]
    Corrupt { column: &'static str, value: String },
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

pub(crate) fn not_found(kind: &'static str, id: impl std::fmt::Display) -> StoreError {
    StoreError::NotFound {
        kind,
        id: id.to_string(),
    }
}

pub(crate) fn corrupt(column: &'static str, value: impl Into<String>) -> StoreError {
    StoreError::Corrupt {
        column,
        value: value.into(),
    }
}
