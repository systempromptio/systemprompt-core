//! Unit tests for QueryExecutorError and QueryExecutor

use systemprompt_database::QueryExecutorError;

// ============================================================================
// QueryExecutorError Display Tests
// ============================================================================

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
    let display = error.to_string();

    assert!(display.contains("WITH"));
}

#[test]
fn test_write_query_not_allowed_mentions_explain() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let display = error.to_string();

    assert!(display.contains("EXPLAIN"));
}

#[test]
fn test_write_query_not_allowed_mentions_pragma() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let display = error.to_string();

    assert!(display.contains("PRAGMA"));
}

// ============================================================================
// QueryExecutorError Debug Tests
// ============================================================================

#[test]
fn test_write_query_not_allowed_debug() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let debug = format!("{:?}", error);

    assert!(debug.contains("WriteQueryNotAllowed"));
}

// ============================================================================
// QueryExecutorError Variant Tests
// ============================================================================

#[test]
fn test_write_query_not_allowed_is_error() {
    let error = QueryExecutorError::WriteQueryNotAllowed;
    let _: &dyn std::error::Error = &error;
}

#[test]
fn test_query_executor_error_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<QueryExecutorError>();
}

#[test]
fn test_query_executor_error_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<QueryExecutorError>();
}

// ============================================================================
// QueryExecutor Type Tests
// ============================================================================

#[test]
fn test_query_executor_debug() {
    use systemprompt_database::QueryExecutor;
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<QueryExecutor>();
}
