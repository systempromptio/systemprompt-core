//! Unit tests for DatabaseQuery, QueryResult, and QuerySelector

use std::collections::HashMap;
use systemprompt_database::{DatabaseQuery, QueryResult, QuerySelector};

// ============================================================================
// DatabaseQuery Tests
// ============================================================================

#[test]
fn test_database_query_new() {
    const QUERY: DatabaseQuery = DatabaseQuery::new("SELECT * FROM users");
    assert_eq!(QUERY.postgres(), "SELECT * FROM users");
}

#[test]
fn test_database_query_const() {
    const SELECT_USERS: DatabaseQuery = DatabaseQuery::new("SELECT id, name FROM users");
    assert_eq!(SELECT_USERS.postgres(), "SELECT id, name FROM users");
}

#[test]
fn test_database_query_complex() {
    const QUERY: DatabaseQuery = DatabaseQuery::new(
        "SELECT u.id, u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id WHERE u.active = true"
    );
    assert!(QUERY.postgres().contains("JOIN"));
    assert!(QUERY.postgres().contains("WHERE"));
}

#[test]
fn test_database_query_with_placeholders() {
    const QUERY: DatabaseQuery = DatabaseQuery::new("SELECT * FROM users WHERE id = $1 AND status = $2");
    assert!(QUERY.postgres().contains("$1"));
    assert!(QUERY.postgres().contains("$2"));
}

#[test]
fn test_database_query_copy() {
    const ORIGINAL: DatabaseQuery = DatabaseQuery::new("SELECT 1");
    let copy = ORIGINAL;
    assert_eq!(ORIGINAL.postgres(), copy.postgres());
}

#[test]
fn test_database_query_debug() {
    const QUERY: DatabaseQuery = DatabaseQuery::new("SELECT * FROM test");
    let debug = format!("{:?}", QUERY);
    assert!(debug.contains("DatabaseQuery"));
}

// ============================================================================
// QuerySelector Trait Tests
// ============================================================================

#[test]
fn test_query_selector_for_str() {
    let query: &str = "SELECT * FROM users";
    assert_eq!(query.select_query(), "SELECT * FROM users");
}

#[test]
fn test_query_selector_for_string() {
    let query = String::from("SELECT id FROM posts");
    assert_eq!(query.select_query(), "SELECT id FROM posts");
}

#[test]
fn test_query_selector_for_database_query() {
    const QUERY: DatabaseQuery = DatabaseQuery::new("SELECT name FROM products");
    assert_eq!(QUERY.select_query(), "SELECT name FROM products");
}

#[test]
fn test_query_selector_empty_string() {
    let query = "";
    assert_eq!(query.select_query(), "");
}

#[test]
fn test_query_selector_whitespace() {
    let query = "   SELECT * FROM users   ";
    assert_eq!(query.select_query(), "   SELECT * FROM users   ");
}

// ============================================================================
// QueryResult Tests
// ============================================================================

#[test]
fn test_query_result_new() {
    let result = QueryResult::new();
    assert!(result.columns.is_empty());
    assert!(result.rows.is_empty());
    assert_eq!(result.row_count, 0);
    assert_eq!(result.execution_time_ms, 0);
}

#[test]
fn test_query_result_default() {
    let result = QueryResult::default();
    assert!(result.columns.is_empty());
    assert!(result.rows.is_empty());
    assert_eq!(result.row_count, 0);
    assert_eq!(result.execution_time_ms, 0);
}

#[test]
fn test_query_result_is_empty_true() {
    let result = QueryResult::new();
    assert!(result.is_empty());
}

#[test]
fn test_query_result_is_empty_false() {
    let mut row = HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));

    let result = QueryResult {
        columns: vec!["id".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 10,
    };

    assert!(!result.is_empty());
}

#[test]
fn test_query_result_first_none() {
    let result = QueryResult::new();
    assert!(result.first().is_none());
}

#[test]
fn test_query_result_first_some() {
    let mut row = HashMap::new();
    row.insert("name".to_string(), serde_json::json!("Alice"));

    let result = QueryResult {
        columns: vec!["name".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 5,
    };

    let first = result.first();
    assert!(first.is_some());
    assert_eq!(first.unwrap().get("name").unwrap(), &serde_json::json!("Alice"));
}

#[test]
fn test_query_result_first_multiple_rows() {
    let mut row1 = HashMap::new();
    row1.insert("id".to_string(), serde_json::json!(1));

    let mut row2 = HashMap::new();
    row2.insert("id".to_string(), serde_json::json!(2));

    let result = QueryResult {
        columns: vec!["id".to_string()],
        rows: vec![row1, row2],
        row_count: 2,
        execution_time_ms: 15,
    };

    let first = result.first();
    assert!(first.is_some());
    assert_eq!(first.unwrap().get("id").unwrap(), &serde_json::json!(1));
}

#[test]
fn test_query_result_with_multiple_columns() {
    let mut row = HashMap::new();
    row.insert("id".to_string(), serde_json::json!(1));
    row.insert("name".to_string(), serde_json::json!("Test"));
    row.insert("active".to_string(), serde_json::json!(true));

    let result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string(), "active".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 8,
    };

    assert_eq!(result.columns.len(), 3);
    assert!(!result.is_empty());
}

#[test]
fn test_query_result_debug() {
    let result = QueryResult::new();
    let debug = format!("{:?}", result);
    assert!(debug.contains("QueryResult"));
}

#[test]
fn test_query_result_clone() {
    let mut row = HashMap::new();
    row.insert("value".to_string(), serde_json::json!(42));

    let result = QueryResult {
        columns: vec!["value".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 100,
    };

    let cloned = result.clone();
    assert_eq!(result.row_count, cloned.row_count);
    assert_eq!(result.columns, cloned.columns);
    assert_eq!(result.execution_time_ms, cloned.execution_time_ms);
}

#[test]
fn test_query_result_serialization() {
    let result = QueryResult {
        columns: vec!["col1".to_string()],
        rows: vec![],
        row_count: 0,
        execution_time_ms: 50,
    };

    let json = serde_json::to_string(&result).expect("Should serialize");
    assert!(json.contains("\"columns\":[\"col1\"]"));
    assert!(json.contains("\"row_count\":0"));
    assert!(json.contains("\"execution_time_ms\":50"));
}

#[test]
fn test_query_result_deserialization() {
    let json = r#"{"columns":["id"],"rows":[],"row_count":0,"execution_time_ms":25}"#;
    let result: QueryResult = serde_json::from_str(json).expect("Should deserialize");

    assert_eq!(result.columns, vec!["id"]);
    assert!(result.rows.is_empty());
    assert_eq!(result.row_count, 0);
    assert_eq!(result.execution_time_ms, 25);
}

#[test]
fn test_query_result_large_row_count() {
    let result = QueryResult {
        columns: vec!["id".to_string()],
        rows: vec![],
        row_count: 1_000_000,
        execution_time_ms: 5000,
    };

    assert_eq!(result.row_count, 1_000_000);
}

#[test]
fn test_query_result_with_null_values() {
    let mut row = HashMap::new();
    row.insert("nullable_field".to_string(), serde_json::Value::Null);

    let result = QueryResult {
        columns: vec!["nullable_field".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 3,
    };

    let first = result.first().unwrap();
    assert_eq!(first.get("nullable_field").unwrap(), &serde_json::Value::Null);
}

#[test]
fn test_query_result_with_nested_json() {
    let mut row = HashMap::new();
    row.insert("data".to_string(), serde_json::json!({
        "nested": {
            "value": 123
        }
    }));

    let result = QueryResult {
        columns: vec!["data".to_string()],
        rows: vec![row],
        row_count: 1,
        execution_time_ms: 7,
    };

    let first = result.first().unwrap();
    let data = first.get("data").unwrap();
    assert!(data.is_object());
}
