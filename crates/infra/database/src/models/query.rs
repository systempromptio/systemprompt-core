use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgRow;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseQuery {
    postgres: &'static str,
}

impl DatabaseQuery {
    #[must_use]
    pub const fn new(query: &'static str) -> Self {
        Self { postgres: query }
    }

    #[must_use]
    pub const fn postgres(&self) -> &str {
        self.postgres
    }
}

pub trait QuerySelector: Sync {
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

pub trait FromDatabaseRow: Sized {
    fn from_postgres_row(row: &PgRow) -> Result<Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<QueryRow>,
    pub row_count: usize,
    pub execution_time_ms: u64,
}

pub type QueryRow = HashMap<String, Value>;

impl QueryResult {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time_ms: 0,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

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
