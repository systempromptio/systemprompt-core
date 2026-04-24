//! Unit tests for QueryExecutorError display

use systemprompt_database::QueryExecutorError;

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
