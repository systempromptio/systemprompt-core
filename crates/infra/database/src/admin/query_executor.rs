//! Query executor used by the CLI's `infra db query` and `db exec` commands.
//!
//! Part of the documented sqlx allowlist: SQL is supplied dynamically by
//! the operator and validated through [`AdminSql`]. Read-only statements run
//! inside a `READ ONLY` transaction so Postgres refuses the writes the parse
//! cannot see (a volatile function that writes).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Column, Row};
use thiserror::Error;

use crate::admin::admin_sql::{AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT};
use crate::models::QueryResult;
use crate::services::postgres::conversion::row_to_json;

#[derive(Error, Debug)]
pub enum QueryExecutorError {
    #[error("Invalid admin SQL: {0}")]
    InvalidSql(#[from] AdminSqlError),

    #[error("Query execution failed: {0}")]
    ExecutionFailed(#[from] sqlx::Error),
}

#[derive(Debug)]
pub struct QueryExecutor {
    pool: Arc<PgPool>,
}

impl QueryExecutor {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn execute_readonly(
        &self,
        raw_sql: &str,
        row_limit: Option<usize>,
    ) -> Result<QueryResult, QueryExecutorError> {
        let sql = AdminSql::parse_readonly(raw_sql)?;
        let start = std::time::Instant::now();
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(tx.as_mut())
            .await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(tx.as_mut())
            .await?;
        tx.rollback().await?;
        Ok(to_result(
            &rows,
            row_limit.unwrap_or(DEFAULT_READONLY_ROW_LIMIT),
            start,
        ))
    }

    pub async fn execute_write(&self, raw_sql: &str) -> Result<QueryResult, QueryExecutorError> {
        let sql = AdminSql::parse_unrestricted(raw_sql)?;
        let start = std::time::Instant::now();
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(&*self.pool)
            .await?;
        Ok(to_result(&rows, usize::MAX, start))
    }
}

fn to_result(rows: &[PgRow], row_limit: usize, start: std::time::Instant) -> QueryResult {
    let execution_time = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let columns = rows.first().map_or_else(Vec::new, |first_row| {
        first_row
            .columns()
            .iter()
            .map(|c| c.name().to_owned())
            .collect()
    });
    let total_rows = rows.len();
    let result_rows = rows.iter().take(row_limit).map(row_to_json).collect();
    QueryResult {
        columns,
        rows: result_rows,
        row_count: total_rows,
        execution_time_ms: execution_time,
    }
}
