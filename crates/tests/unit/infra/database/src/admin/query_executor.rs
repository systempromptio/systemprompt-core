//! Unit tests for QueryExecutorError display plus DB-backed executor paths.

use std::sync::Arc;

use systemprompt_database::{QueryExecutor, QueryExecutorError};

use crate::services::db_helper::pool;

async fn executor() -> Option<QueryExecutor> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some(QueryExecutor::new(Arc::clone(&pg)))
}

#[tokio::test]
async fn execute_readonly_extracts_typed_columns_as_json() {
    let Some(exec) = executor().await else { return };

    let result = exec
        .execute_readonly(
            "SELECT 'txt'::text AS s, 42::bigint AS big, 7::int AS small, 1.5::float8 AS f, true \
             AS b, ARRAY['x','y']::text[] AS arr, '{\"k\":1}'::jsonb AS j, NULL::text AS n, \
             TIMESTAMPTZ '2026-01-02 03:04:05+00' AS ts",
            None,
        )
        .await
        .expect("readonly select");

    assert_eq!(result.row_count, 1);
    let row = result.rows.first().expect("one row");
    assert_eq!(row["s"], serde_json::json!("txt"));
    assert_eq!(row["big"], serde_json::json!(42));
    assert_eq!(row["small"], serde_json::json!(7));
    assert_eq!(row["f"], serde_json::json!(1.5));
    assert_eq!(row["b"], serde_json::json!(true));
    assert_eq!(row["arr"], serde_json::json!(["x", "y"]));
    assert_eq!(row["j"], serde_json::json!({"k": 1}));
    assert_eq!(row["n"], serde_json::Value::Null);
    assert!(
        row["ts"]
            .as_str()
            .is_some_and(|ts| ts.starts_with("2026-01-02T03:04:05"))
    );
    assert!(result.columns.contains(&"arr".to_owned()));
}

#[tokio::test]
async fn execute_readonly_caps_rows_but_reports_total_count() {
    let Some(exec) = executor().await else { return };

    let result = exec
        .execute_readonly("SELECT generate_series(1, 5) AS n", Some(2))
        .await
        .expect("capped select");

    assert_eq!(result.row_count, 5);
    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn execute_readonly_rejects_write_statements() {
    let Some(exec) = executor().await else { return };

    let err = exec
        .execute_readonly("DELETE FROM users", None)
        .await
        .expect_err("write rejected");
    assert!(matches!(err, QueryExecutorError::InvalidSql(_)));
}

#[tokio::test]
async fn execute_write_runs_ddl_and_dml() {
    let Some(exec) = executor().await else { return };
    let table = format!("qexec_{}", uuid::Uuid::new_v4().simple());

    exec.execute_write(&format!("CREATE TABLE \"{table}\" (id BIGINT PRIMARY KEY)"))
        .await
        .expect("ddl");
    exec.execute_write(&format!("INSERT INTO \"{table}\" (id) VALUES (1), (2)"))
        .await
        .expect("dml");

    let result = exec
        .execute_readonly(&format!("SELECT COUNT(*) AS c FROM \"{table}\""), None)
        .await
        .expect("count");
    assert_eq!(result.rows[0]["c"], serde_json::json!(2));

    let _ = exec.execute_write(&format!("DROP TABLE \"{table}\"")).await;
}

#[tokio::test]
async fn execute_write_rejects_multiple_statements() {
    let Some(exec) = executor().await else { return };

    let err = exec
        .execute_write("SELECT 1; SELECT 2")
        .await
        .expect_err("multi-statement rejected");
    assert!(matches!(err, QueryExecutorError::InvalidSql(_)));
}

#[tokio::test]
async fn execute_readonly_maps_bad_sql_to_execution_failure() {
    let Some(exec) = executor().await else { return };

    let err = exec
        .execute_readonly("SELECT * FROM table_that_does_not_exist_qq", None)
        .await
        .expect_err("bad relation");
    assert!(matches!(err, QueryExecutorError::ExecutionFailed(_)));
}

#[test]
fn test_write_query_not_allowed_display() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let display = error.to_string();

    assert!(display.contains("Write query not allowed"));
    assert!(display.contains("read-only mode"));
    assert!(display.contains("SELECT"));
}

#[test]
fn test_write_query_not_allowed_mentions_with() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    assert!(error.to_string().contains("WITH"));
}

#[test]
fn test_write_query_not_allowed_mentions_explain() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    assert!(error.to_string().contains("EXPLAIN"));
}

#[test]
fn test_write_query_not_allowed_mentions_show() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    assert!(error.to_string().contains("SHOW"));
}

#[test]
fn test_write_query_not_allowed_debug() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let debug = format!("{:?}", error);
    assert!(debug.contains("WriteQueryNotAllowed"));
}
