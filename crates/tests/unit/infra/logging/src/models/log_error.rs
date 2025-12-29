//! Unit tests for LoggingError enum

use systemprompt_core_logging::models::LoggingError;

// ============================================================================
// LoggingError Constructor Tests
// ============================================================================

#[test]
fn test_logging_error_invalid_log_entry() {
    let error = LoggingError::invalid_log_entry("timestamp", "invalid format");

    assert_eq!(
        error.to_string(),
        "Invalid log entry: timestamp invalid format"
    );
}

#[test]
fn test_logging_error_validation_error() {
    let error = LoggingError::validation_error("Missing required field");

    assert_eq!(
        error.to_string(),
        "Log entry validation failed: Missing required field"
    );
}

#[test]
fn test_logging_error_invalid_log_level() {
    let error = LoggingError::invalid_log_level("CRITICAL");

    assert_eq!(error.to_string(), "Invalid log level: CRITICAL");
}

#[test]
fn test_logging_error_log_entry_not_found() {
    let error = LoggingError::log_entry_not_found("log-123-456");

    assert_eq!(error.to_string(), "Log entry not found: log-123-456");
}

#[test]
fn test_logging_error_repository_error() {
    let error = LoggingError::repository_error("insert failed");

    assert_eq!(
        error.to_string(),
        "Log repository operation failed: insert failed"
    );
}

#[test]
fn test_logging_error_cleanup_error() {
    let error = LoggingError::cleanup_error(42);

    assert_eq!(
        error.to_string(),
        "Cleanup operation failed: deleted 42 entries"
    );
}

#[test]
fn test_logging_error_pagination_error() {
    let error = LoggingError::pagination_error(-1, 0);

    assert_eq!(
        error.to_string(),
        "Pagination parameters invalid: page=-1, per_page=0"
    );
}

#[test]
fn test_logging_error_filter_error() {
    let error = LoggingError::filter_error("level", "UNKNOWN");

    assert_eq!(error.to_string(), "Log filter invalid: level=UNKNOWN");
}

// ============================================================================
// LoggingError Variant Display Tests
// ============================================================================

#[test]
fn test_logging_error_empty_module_name() {
    let error = LoggingError::EmptyModuleName;

    assert_eq!(error.to_string(), "Empty log module name");
}

#[test]
fn test_logging_error_empty_message() {
    let error = LoggingError::EmptyMessage;

    assert_eq!(error.to_string(), "Empty log message");
}

#[test]
fn test_logging_error_invalid_metadata() {
    let error = LoggingError::InvalidMetadata;

    assert_eq!(error.to_string(), "Invalid metadata format");
}

#[test]
fn test_logging_error_terminal_error() {
    let error = LoggingError::TerminalError;

    assert_eq!(error.to_string(), "Terminal output failed");
}

#[test]
fn test_logging_error_database_unavailable() {
    let error = LoggingError::DatabaseUnavailable;

    assert_eq!(error.to_string(), "Database connection not available");
}

// ============================================================================
// LoggingError Debug Tests
// ============================================================================

#[test]
fn test_logging_error_debug_invalid_log_entry() {
    let error = LoggingError::invalid_log_entry("field", "reason");
    let debug = format!("{:?}", error);

    assert!(debug.contains("InvalidLogEntry"));
    assert!(debug.contains("field"));
    assert!(debug.contains("reason"));
}

#[test]
fn test_logging_error_debug_validation_error() {
    let error = LoggingError::validation_error("message");
    let debug = format!("{:?}", error);

    assert!(debug.contains("ValidationError"));
    assert!(debug.contains("message"));
}

#[test]
fn test_logging_error_debug_empty_module_name() {
    let error = LoggingError::EmptyModuleName;
    let debug = format!("{:?}", error);

    assert!(debug.contains("EmptyModuleName"));
}

// ============================================================================
// LoggingError Constructor with String Types Tests
// ============================================================================

#[test]
fn test_logging_error_invalid_log_entry_with_string() {
    let error = LoggingError::invalid_log_entry(String::from("field"), String::from("reason"));

    assert!(error.to_string().contains("field"));
    assert!(error.to_string().contains("reason"));
}

#[test]
fn test_logging_error_validation_error_with_string() {
    let error = LoggingError::validation_error(String::from("Validation failed"));

    assert!(error.to_string().contains("Validation failed"));
}

#[test]
fn test_logging_error_invalid_log_level_with_string() {
    let error = LoggingError::invalid_log_level(String::from("INVALID_LEVEL"));

    assert!(error.to_string().contains("INVALID_LEVEL"));
}

#[test]
fn test_logging_error_log_entry_not_found_with_string() {
    let error = LoggingError::log_entry_not_found(String::from("entry-id-123"));

    assert!(error.to_string().contains("entry-id-123"));
}

#[test]
fn test_logging_error_repository_error_with_string() {
    let error = LoggingError::repository_error(String::from("operation failed"));

    assert!(error.to_string().contains("operation failed"));
}

#[test]
fn test_logging_error_filter_error_with_string() {
    let error = LoggingError::filter_error(String::from("module"), String::from("invalid"));

    assert!(error.to_string().contains("module"));
    assert!(error.to_string().contains("invalid"));
}

// ============================================================================
// LoggingError into_sqlx_error Tests
// ============================================================================

#[test]
fn test_logging_error_into_sqlx_error() {
    let error = LoggingError::EmptyModuleName;
    let sqlx_error = error.into_sqlx_error();

    // sqlx::Error::Protocol contains the error message
    let error_str = sqlx_error.to_string();
    assert!(error_str.contains("Empty log module name"));
}

#[test]
fn test_logging_error_into_sqlx_error_validation() {
    let error = LoggingError::validation_error("test validation");
    let sqlx_error = error.into_sqlx_error();

    let error_str = sqlx_error.to_string();
    assert!(error_str.contains("test validation"));
}

// ============================================================================
// LoggingError Edge Cases
// ============================================================================

#[test]
fn test_logging_error_with_empty_strings() {
    let error = LoggingError::invalid_log_entry("", "");
    assert_eq!(error.to_string(), "Invalid log entry:  ");
}

#[test]
fn test_logging_error_with_special_characters() {
    let error = LoggingError::validation_error("Error: <script>alert('xss')</script>");

    assert!(error.to_string().contains("<script>"));
}

#[test]
fn test_logging_error_cleanup_error_zero() {
    let error = LoggingError::cleanup_error(0);

    assert_eq!(
        error.to_string(),
        "Cleanup operation failed: deleted 0 entries"
    );
}

#[test]
fn test_logging_error_cleanup_error_max() {
    let error = LoggingError::cleanup_error(u64::MAX);

    assert!(error.to_string().contains(&u64::MAX.to_string()));
}

#[test]
fn test_logging_error_pagination_boundary_values() {
    let error = LoggingError::pagination_error(i32::MIN, i32::MAX);

    assert!(error.to_string().contains(&i32::MIN.to_string()));
    assert!(error.to_string().contains(&i32::MAX.to_string()));
}

#[test]
fn test_logging_error_with_long_message() {
    let long_message = "x".repeat(10000);
    let error = LoggingError::validation_error(&long_message);

    assert!(error.to_string().contains(&long_message));
}

#[test]
fn test_logging_error_with_newlines() {
    let error = LoggingError::validation_error("Line 1\nLine 2\nLine 3");

    assert!(error.to_string().contains("Line 1\nLine 2\nLine 3"));
}

#[test]
fn test_logging_error_with_unicode() {
    let error = LoggingError::validation_error("Error with emoji and unicode");

    assert!(error.to_string().contains("emoji"));
}
