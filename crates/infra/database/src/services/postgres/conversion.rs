use sqlx::{Column, Row};
use std::collections::HashMap;

use crate::models::{DbValue, QueryResult, ToDbValue};

pub fn rows_to_result(rows: Vec<sqlx::postgres::PgRow>, start: std::time::Instant) -> QueryResult {
    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        columns = first_row
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
    }

    for row in rows {
        result_rows.push(row_to_json(&row));
    }

    let row_count = result_rows.len();
    let execution_time_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

    QueryResult {
        columns,
        rows: result_rows,
        row_count,
        execution_time_ms,
    }
}

pub fn row_to_json(row: &sqlx::postgres::PgRow) -> HashMap<String, serde_json::Value> {
    row.columns()
        .iter()
        .map(|col| (col.name().to_string(), column_to_json(row, col.ordinal())))
        .collect()
}

fn column_to_json(row: &sqlx::postgres::PgRow, ordinal: usize) -> serde_json::Value {
    if let Ok(val) = row.try_get::<Option<chrono::NaiveDateTime>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::String(v.and_utc().to_rfc3339())
        });
    }
    if let Ok(val) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::String(v.to_rfc3339())
        });
    }
    if let Ok(val) = row.try_get::<Option<uuid::Uuid>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::String(v.to_string())
        });
    }
    if let Ok(val) = row.try_get::<Option<String>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, serde_json::Value::String);
    }
    if let Ok(val) = row.try_get::<Option<i64>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::Number(v.into())
        });
    }
    if let Ok(val) = row.try_get::<Option<i32>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::Number(i64::from(v).into())
        });
    }
    if let Ok(val) = row.try_get::<Option<f64>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| serde_json::json!(v));
    }
    if let Ok(val) = row.try_get::<Option<rust_decimal::Decimal>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            v.to_string().parse::<f64>().map_or_else(
                |_| serde_json::Value::String(v.to_string()),
                |f| serde_json::json!(f),
            )
        });
    }
    if let Ok(val) = row.try_get::<Option<bool>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, serde_json::Value::Bool);
    }
    if let Ok(val) = row.try_get::<Option<Vec<String>>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |v| {
            serde_json::Value::Array(v.into_iter().map(serde_json::Value::String).collect())
        });
    }
    if let Ok(val) = row.try_get::<Option<serde_json::Value>, _>(ordinal) {
        return val.unwrap_or(serde_json::Value::Null);
    }
    if let Ok(val) = row.try_get::<Option<Vec<u8>>, _>(ordinal) {
        return val.map_or(serde_json::Value::Null, |bytes| {
            use base64::engine::general_purpose::STANDARD;
            use base64::Engine;
            serde_json::Value::String(STANDARD.encode(&bytes))
        });
    }
    serde_json::Value::Null
}

pub fn bind_params<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    params: &[&dyn ToDbValue],
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    for param in params {
        let value = param.to_db_value();
        query = match value {
            DbValue::String(s) => query.bind(s),
            DbValue::Int(i) => query.bind(i),
            DbValue::Float(f) => query.bind(f),
            DbValue::Bool(b) => query.bind(b),
            DbValue::Bytes(b) => query.bind(b),
            DbValue::Timestamp(dt) => query.bind(dt),
            DbValue::StringArray(arr) => query.bind(arr),
            DbValue::NullString => query.bind(None::<String>),
            DbValue::NullInt => query.bind(None::<i64>),
            DbValue::NullFloat => query.bind(None::<f64>),
            DbValue::NullBool => query.bind(None::<bool>),
            DbValue::NullBytes => query.bind(None::<Vec<u8>>),
            DbValue::NullTimestamp => query.bind(None::<chrono::DateTime<chrono::Utc>>),
            DbValue::NullStringArray => query.bind(None::<Vec<String>>),
        };
    }
    query
}
