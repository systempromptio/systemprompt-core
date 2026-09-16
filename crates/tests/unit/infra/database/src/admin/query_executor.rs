//! Unit tests for QueryExecutorError display plus DB-backed executor paths.

use std::sync::Arc;

use systemprompt_database::{QueryExecutor, QueryExecutorError};

use crate::services::db_helper::pool_or_skip;

async fn executor_or_skip() -> Option<QueryExecutor> {
    let db = pool_or_skip().await?;
    let pg = db.write_pool_arc().ok()?;
    Some(QueryExecutor::new(Arc::clone(&pg)))
}

#[tokio::test]
async fn execute_readonly_extracts_typed_columns_as_json() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };

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
    let Some(exec) = executor_or_skip().await else {
        return;
    };

    let result = exec
        .execute_readonly("SELECT generate_series(1, 5) AS n", Some(2))
        .await
        .expect("capped select");

    assert_eq!(result.row_count, 5);
    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn execute_readonly_rejects_write_statements() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };

    let err = exec
        .execute_readonly("DELETE FROM users", None)
        .await
        .expect_err("write rejected");
    assert!(matches!(err, QueryExecutorError::InvalidSql(_)));
}

#[tokio::test]
async fn execute_write_runs_ddl_and_dml() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };
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
    let Some(exec) = executor_or_skip().await else {
        return;
    };

    let err = exec
        .execute_write("SELECT 1; SELECT 2")
        .await
        .expect_err("multi-statement rejected");
    assert!(matches!(err, QueryExecutorError::InvalidSql(_)));
}

#[tokio::test]
async fn execute_readonly_maps_bad_sql_to_execution_failure() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };

    let err = exec
        .execute_readonly("SELECT * FROM table_that_does_not_exist_qq", None)
        .await
        .expect_err("bad relation");
    assert!(matches!(err, QueryExecutorError::ExecutionFailed(_)));
}


#[tokio::test]
async fn execute_readonly_decodes_uuid_numeric_and_bytea_columns() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };

    let result = exec
        .execute_readonly(
            "SELECT gen_random_uuid() AS u, 1.5::numeric AS n, '\\xdead'::bytea AS b",
            None,
        )
        .await
        .expect("query runs");
    let row = result.rows.first().expect("one row");

    assert!(
        row["u"].as_str().is_some_and(|u| u.len() == 36),
        "uuid must decode to its text form, got {:?}",
        row["u"]
    );
    assert_eq!(
        row["n"].as_f64(),
        Some(1.5),
        "numeric must not read as NULL"
    );
    assert!(
        row["b"].as_str().is_some(),
        "bytea must decode to base64 text, got {:?}",
        row["b"]
    );
}

#[tokio::test]
async fn execute_readonly_refuses_a_write_hidden_in_a_volatile_function() {
    let Some(exec) = executor_or_skip().await else {
        return;
    };
    let table = format!("ro_probe_{}", uuid::Uuid::new_v4().simple());
    exec.execute_write(&format!("CREATE TABLE \"{table}\" (id INT)"))
        .await
        .expect("create");
    let fn_name = format!("{table}_write");
    exec.execute_write(&format!(
        "CREATE FUNCTION \"{fn_name}\"() RETURNS INT LANGUAGE sql AS $$ INSERT INTO \"{table}\" \
         VALUES (1) RETURNING id $$"
    ))
    .await
    .expect("create fn");

    let err = exec
        .execute_readonly(&format!("SELECT \"{fn_name}\"()"), None)
        .await
        .expect_err("a READ ONLY transaction refuses the insert");
    assert!(matches!(err, QueryExecutorError::ExecutionFailed(_)));

    let count = exec
        .execute_readonly(&format!("SELECT count(*) AS c FROM \"{table}\""), None)
        .await
        .expect("count");
    assert_eq!(count.rows[0]["c"].as_i64(), Some(0));

    let _ = exec
        .execute_write(&format!("DROP FUNCTION \"{fn_name}\"()"))
        .await;
    let _ = exec.execute_write(&format!("DROP TABLE \"{table}\"")).await;
}
