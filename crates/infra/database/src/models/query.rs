//! Query primitives: selectors, dynamic results, typed row decoding.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgRow;
use std::collections::HashMap;

/// Wrapper around a static, validated SQL string. Used by extension code that
/// wants to ship pre-built queries through the [`QuerySelector`] interface
/// without re-allocating per call.
#[derive(Debug, Clone, Copy)]
pub struct DatabaseQuery {
    postgres: &'static str,
}

impl DatabaseQuery {
    /// Wrap a `'static` SQL string.
    #[must_use]
    pub const fn new(query: &'static str) -> Self {
        Self { postgres: query }
    }

    /// Borrow the wrapped SQL string.
    #[must_use]
    pub const fn postgres(&self) -> &str {
        self.postgres
    }
}

/// Trait implemented by anything that can supply a SQL string at runtime.
/// Implemented for [`&str`], [`String`], and [`DatabaseQuery`].
pub trait QuerySelector: Sync {
    /// Borrow the underlying SQL.
    fn select_query(&self) -> &str;
}

impl QuerySelector for &str {
    fn select_query(&self) -> &str {
        self
    }
}

impl QuerySelector for String {
    fn select_query(&self) -> &str {
        self.as_str()
    }
}

impl QuerySelector for DatabaseQuery {
    fn select_query(&self) -> &str {
        self.postgres()
    }
}

/// Decode a typed value from a single `PostgreSQL` row.
///
/// Implemented by domain models that opt in to the typed-row helpers on
/// [`crate::services::DatabaseProviderExt`].
pub trait FromDatabaseRow: Sized {
    /// Decode `Self` from a `PostgreSQL` row.
    fn from_postgres_row(row: &PgRow) -> Result<Self>;
}

/// Result of a dynamic query: column ordering, JSON-typed rows, total count
/// and execution time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Column names in the order they were returned by the server.
    pub columns: Vec<String>,
    /// Rows returned by the query. Each row is keyed by column name.
    pub rows: Vec<QueryRow>,
    /// Number of rows returned (may differ from `rows.len()` once a row limit
    /// has been applied by the executor).
    pub row_count: usize,
    /// Wall-clock execution time in milliseconds.
    pub execution_time_ms: u64,
}

/// Dynamic query result row, keyed by column name. JSON typing is intentional
/// — the column shape is unknown at compile time.
pub type QueryRow = HashMap<String, Value>;

impl QueryResult {
    /// Construct an empty [`QueryResult`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time_ms: 0,
        }
    }

    /// Returns true when no rows were returned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Borrow the first row, if any.
    #[must_use]
    pub fn first(&self) -> Option<&QueryRow> {
        self.rows.first()
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}
