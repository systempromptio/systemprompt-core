use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::PgPool;
use sqlx::{Column, Row};
use thiserror::Error;

use crate::models::QueryResult;

#[derive(Error, Debug)]
pub enum QueryExecutorError {
    #[error(
        "Write query not allowed in read-only mode: only SELECT, WITH, EXPLAIN, and PRAGMA \
         queries are permitted"
    )]
    WriteQueryNotAllowed,

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

    pub async fn execute_query(
        &self,
        query: &str,
        read_only: bool,
    ) -> Result<QueryResult, QueryExecutorError> {
        let start = std::time::Instant::now();

        if read_only && !Self::is_safe_query(query) {
            return Err(QueryExecutorError::WriteQueryNotAllowed);
        }

        let rows = sqlx::query(query).fetch_all(&*self.pool).await?;
        let execution_time = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let mut columns = Vec::new();
        let mut result_rows = Vec::new();

        if let Some(first_row) = rows.first() {
            columns = first_row
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect();
        }

        for row in &rows {
            let mut row_map = HashMap::new();
            for (i, column) in row.columns().iter().enumerate() {
                row_map.insert(column.name().to_string(), Self::extract_value(row, i));
            }
            result_rows.push(row_map);
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
            row_count: rows.len(),
            execution_time_ms: execution_time,
        })
    }

    fn is_safe_query(query: &str) -> bool {
        let trimmed = query.trim().to_lowercase();
        let safe_starts = ["select", "with", "explain", "pragma"];
        let unsafe_ops = [
            " drop ", " delete ", " insert ", " update ", " alter ", " create ",
        ];

        safe_starts.iter().any(|s| trimmed.starts_with(s))
            && !unsafe_ops.iter().any(|op| trimmed.contains(op))
    }

    fn extract_value(row: &sqlx::postgres::PgRow, column_index: usize) -> serde_json::Value {
        if let Ok(val) = row.try_get::<Option<chrono::NaiveDateTime>, _>(column_index) {
            return val.map_or(serde_json::Value::Null, |dt| {
                serde_json::Value::String(dt.and_utc().to_rfc3339())
            });
        }
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
}
