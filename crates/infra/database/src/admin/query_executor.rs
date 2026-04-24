use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::PgPool;
use sqlx::{Column, Row};
use thiserror::Error;

use crate::admin::admin_sql::{AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT};
use crate::models::QueryResult;

#[derive(Error, Debug)]
pub enum QueryExecutorError {
    #[error(
        "Write query not allowed in read-only mode: only SELECT, WITH, EXPLAIN, SHOW, TABLE, and \
         VALUES queries are permitted"
    )]
    WriteQueryNotAllowed,

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
        self.execute(sql, row_limit.unwrap_or(DEFAULT_READONLY_ROW_LIMIT))
            .await
    }

    pub async fn execute_write(&self, raw_sql: &str) -> Result<QueryResult, QueryExecutorError> {
        let sql = AdminSql::parse_unrestricted(raw_sql)?;
        self.execute(sql, usize::MAX).await
    }

    async fn execute(
        &self,
        sql: AdminSql,
        row_limit: usize,
    ) -> Result<QueryResult, QueryExecutorError> {
        let start = std::time::Instant::now();

        let rows = sqlx::query(sql.as_str()).fetch_all(&*self.pool).await?;
        let execution_time = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let columns = rows.first().map_or_else(Vec::new, |first_row| {
            first_row
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect()
        });

        let total_rows = rows.len();
        let capped_rows = rows.iter().take(row_limit);
        let mut result_rows = Vec::with_capacity(total_rows.min(row_limit));

        for row in capped_rows {
            let mut row_map = HashMap::new();
            for (i, column) in row.columns().iter().enumerate() {
                row_map.insert(column.name().to_string(), extract_value(row, i));
            }
            result_rows.push(row_map);
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
            row_count: total_rows,
            execution_time_ms: execution_time,
        })
    }
}

fn extract_value(row: &sqlx::postgres::PgRow, column_index: usize) -> serde_json::Value {
    if let Ok(val) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, |dt| {
            serde_json::Value::String(dt.to_rfc3339())
        });
    }
    if let Ok(val) = row.try_get::<Option<String>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, serde_json::Value::String);
    }
    if let Ok(val) = row.try_get::<Option<i64>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, |i| {
            serde_json::Value::Number(i.into())
        });
    }
    if let Ok(val) = row.try_get::<Option<i32>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, |i| {
            serde_json::Value::Number(i.into())
        });
    }
    if let Ok(val) = row.try_get::<Option<f64>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, |f| {
            serde_json::Number::from_f64(f)
                .map_or(serde_json::Value::Null, serde_json::Value::Number)
        });
    }
    if let Ok(val) = row.try_get::<Option<bool>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, serde_json::Value::Bool);
    }
    if let Ok(val) = row.try_get::<Option<Vec<String>>, _>(column_index) {
        return val.map_or(serde_json::Value::Null, |arr| {
            serde_json::Value::Array(arr.into_iter().map(serde_json::Value::String).collect())
        });
    }
    if let Ok(val) = row.try_get::<Option<serde_json::Value>, _>(column_index) {
        return val.unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}
