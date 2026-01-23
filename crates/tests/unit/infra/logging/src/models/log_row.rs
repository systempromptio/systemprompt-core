//! Unit tests for LogRow model

use chrono::Utc;
use systemprompt_identifiers::LogId;
use systemprompt_logging::models::LogRow;
use systemprompt_logging::{LogEntry, LogLevel};

// ============================================================================
// LogRow Creation Tests
// ============================================================================

#[test]
fn test_log_row_creation() {
    let row = LogRow {
        id: LogId::new("log-test-123"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test::module".to_string(),
        message: "Test message".to_string(),
        metadata: Some(r#"{"key": "value"}"#.to_string()),
        user_id: Some("user-123".to_string()),
        session_id: Some("session-456".to_string()),
        task_id: Some("task-789".to_string()),
        trace_id: Some("trace-abc".to_string()),
        context_id: Some("context-def".to_string()),
        client_id: Some("client-ghi".to_string()),
    };

    assert_eq!(row.level, "info");
    assert_eq!(row.module, "test::module");
    assert_eq!(row.message, "Test message");
    assert!(row.metadata.is_some());
    assert!(row.user_id.is_some());
    assert!(row.session_id.is_some());
    assert!(row.task_id.is_some());
    assert!(row.trace_id.is_some());
    assert!(row.context_id.is_some());
    assert!(row.client_id.is_some());
}

#[test]
fn test_log_row_minimal() {
    let row = LogRow {
        id: LogId::new("log-minimal"),
        timestamp: Utc::now(),
        level: "warn".to_string(),
        module: "minimal".to_string(),
        message: "Minimal message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    assert!(row.metadata.is_none());
    assert!(row.user_id.is_none());
    assert!(row.session_id.is_none());
    assert!(row.task_id.is_none());
    assert!(row.trace_id.is_none());
    assert!(row.context_id.is_none());
    assert!(row.client_id.is_none());
}

#[test]
fn test_log_row_debug() {
    let row = LogRow {
        id: LogId::new("debug-row"),
        timestamp: Utc::now(),
        level: "debug".to_string(),
        module: "debug::module".to_string(),
        message: "Debug message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let debug = format!("{:?}", row);
    assert!(debug.contains("LogRow"));
    assert!(debug.contains("debug::module"));
}

// ============================================================================
// LogRow to LogEntry Conversion Tests
// ============================================================================

#[test]
fn test_log_row_to_log_entry_info() {
    let row = LogRow {
        id: LogId::new("log-info"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "conversion::test".to_string(),
        message: "Info conversion test".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();

    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(entry.module, "conversion::test");
    assert_eq!(entry.message, "Info conversion test");
}

#[test]
fn test_log_row_to_log_entry_error() {
    let row = LogRow {
        id: LogId::new("log-error"),
        timestamp: Utc::now(),
        level: "error".to_string(),
        module: "error::module".to_string(),
        message: "Error message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Error);
}

#[test]
fn test_log_row_to_log_entry_warn() {
    let row = LogRow {
        id: LogId::new("log-warn"),
        timestamp: Utc::now(),
        level: "warn".to_string(),
        module: "warn::module".to_string(),
        message: "Warning message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Warn);
}

#[test]
fn test_log_row_to_log_entry_debug() {
    let row = LogRow {
        id: LogId::new("log-debug"),
        timestamp: Utc::now(),
        level: "debug".to_string(),
        module: "debug::module".to_string(),
        message: "Debug message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Debug);
}

#[test]
fn test_log_row_to_log_entry_trace() {
    let row = LogRow {
        id: LogId::new("log-trace"),
        timestamp: Utc::now(),
        level: "trace".to_string(),
        module: "trace::module".to_string(),
        message: "Trace message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Trace);
}

#[test]
fn test_log_row_to_log_entry_invalid_level_defaults_to_info() {
    let row = LogRow {
        id: LogId::new("log-invalid"),
        timestamp: Utc::now(),
        level: "INVALID_LEVEL".to_string(),
        module: "invalid::module".to_string(),
        message: "Invalid level message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Info);
}

#[test]
fn test_log_row_to_log_entry_with_valid_metadata() {
    let row = LogRow {
        id: LogId::new("log-meta"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "meta::module".to_string(),
        message: "Metadata message".to_string(),
        metadata: Some(r#"{"key": "value", "number": 42}"#.to_string()),
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();

    assert!(entry.metadata.is_some());
    let meta = entry.metadata.unwrap();
    assert_eq!(meta["key"], "value");
    assert_eq!(meta["number"], 42);
}

#[test]
fn test_log_row_to_log_entry_with_invalid_metadata_json() {
    let row = LogRow {
        id: LogId::new("log-invalid-meta"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "invalid_meta::module".to_string(),
        message: "Invalid metadata JSON".to_string(),
        metadata: Some("not valid json {{{".to_string()),
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert!(entry.metadata.is_none());
}

#[test]
fn test_log_row_to_log_entry_with_user_id() {
    let row = LogRow {
        id: LogId::new("log-user"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "user::module".to_string(),
        message: "User message".to_string(),
        metadata: None,
        user_id: Some("user-12345".to_string()),
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.user_id.as_str(), "user-12345");
}

#[test]
fn test_log_row_to_log_entry_with_session_id() {
    let row = LogRow {
        id: LogId::new("log-session"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "session::module".to_string(),
        message: "Session message".to_string(),
        metadata: None,
        user_id: None,
        session_id: Some("session-67890".to_string()),
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.session_id.as_str(), "session-67890");
}

#[test]
fn test_log_row_to_log_entry_with_task_id() {
    let row = LogRow {
        id: LogId::new("log-task"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "task::module".to_string(),
        message: "Task message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: Some("task-abcde".to_string()),
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert!(entry.task_id.is_some());
    assert_eq!(entry.task_id.as_ref().unwrap().as_str(), "task-abcde");
}

#[test]
fn test_log_row_to_log_entry_with_trace_id() {
    let row = LogRow {
        id: LogId::new("log-trace-id"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "trace::module".to_string(),
        message: "Trace ID message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: Some("trace-fghij".to_string()),
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.trace_id.as_str(), "trace-fghij");
}

#[test]
fn test_log_row_to_log_entry_with_context_id() {
    let row = LogRow {
        id: LogId::new("log-context"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "context::module".to_string(),
        message: "Context message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: Some("context-klmno".to_string()),
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert!(entry.context_id.is_some());
    assert_eq!(entry.context_id.as_ref().unwrap().as_str(), "context-klmno");
}

#[test]
fn test_log_row_to_log_entry_with_client_id() {
    let row = LogRow {
        id: LogId::new("log-client"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "client::module".to_string(),
        message: "Client message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: Some("client-pqrst".to_string()),
    };

    let entry: LogEntry = row.into();
    assert!(entry.client_id.is_some());
    assert_eq!(entry.client_id.as_ref().unwrap().as_str(), "client-pqrst");
}

#[test]
fn test_log_row_to_log_entry_preserves_timestamp() {
    let timestamp = Utc::now();
    let row = LogRow {
        id: LogId::new("log-ts"),
        timestamp,
        level: "info".to_string(),
        module: "ts::module".to_string(),
        message: "Timestamp message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.timestamp, timestamp);
}

#[test]
fn test_log_row_to_log_entry_preserves_id() {
    let id = LogId::new("log-preserve-id");
    let row = LogRow {
        id: id.clone(),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "id::module".to_string(),
        message: "ID message".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.id, id);
}

#[test]
fn test_log_row_to_log_entry_full_conversion() {
    let row = LogRow {
        id: LogId::new("log-full"),
        timestamp: Utc::now(),
        level: "warn".to_string(),
        module: "full::conversion".to_string(),
        message: "Full conversion test".to_string(),
        metadata: Some(r#"{"test": true}"#.to_string()),
        user_id: Some("user-full".to_string()),
        session_id: Some("session-full".to_string()),
        task_id: Some("task-full".to_string()),
        trace_id: Some("trace-full".to_string()),
        context_id: Some("context-full".to_string()),
        client_id: Some("client-full".to_string()),
    };

    let entry: LogEntry = row.into();

    assert_eq!(entry.level, LogLevel::Warn);
    assert_eq!(entry.module, "full::conversion");
    assert_eq!(entry.message, "Full conversion test");
    assert!(entry.metadata.is_some());
    assert_eq!(entry.user_id.as_str(), "user-full");
    assert_eq!(entry.session_id.as_str(), "session-full");
    assert!(entry.task_id.is_some());
    assert_eq!(entry.trace_id.as_str(), "trace-full");
    assert!(entry.context_id.is_some());
    assert!(entry.client_id.is_some());
}

// ============================================================================
// LogRow Default ID Conversion Tests
// ============================================================================

#[test]
fn test_log_row_to_log_entry_no_user_id_uses_system() {
    let row = LogRow {
        id: LogId::new("log-no-user"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test".to_string(),
        message: "No user".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(
        entry.user_id.as_str(),
        systemprompt_identifiers::UserId::system().as_str()
    );
}

#[test]
fn test_log_row_to_log_entry_no_session_id_uses_system() {
    let row = LogRow {
        id: LogId::new("log-no-session"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test".to_string(),
        message: "No session".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(
        entry.session_id.as_str(),
        systemprompt_identifiers::SessionId::system().as_str()
    );
}

#[test]
fn test_log_row_to_log_entry_no_trace_id_uses_system() {
    let row = LogRow {
        id: LogId::new("log-no-trace"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test".to_string(),
        message: "No trace".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(
        entry.trace_id.as_str(),
        systemprompt_identifiers::TraceId::system().as_str()
    );
}

// ============================================================================
// LogRow Level String Variations Tests
// ============================================================================

#[test]
fn test_log_row_level_case_insensitive_info() {
    let row = LogRow {
        id: LogId::new("log-info-caps"),
        timestamp: Utc::now(),
        level: "INFO".to_string(),
        module: "test".to_string(),
        message: "INFO caps".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Info);
}

#[test]
fn test_log_row_level_case_insensitive_error() {
    let row = LogRow {
        id: LogId::new("log-error-caps"),
        timestamp: Utc::now(),
        level: "ERROR".to_string(),
        module: "test".to_string(),
        message: "ERROR caps".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Error);
}

#[test]
fn test_log_row_level_case_insensitive_warn() {
    let row = LogRow {
        id: LogId::new("log-warn-caps"),
        timestamp: Utc::now(),
        level: "WARN".to_string(),
        module: "test".to_string(),
        message: "WARN caps".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Warn);
}

#[test]
fn test_log_row_level_case_insensitive_debug() {
    let row = LogRow {
        id: LogId::new("log-debug-caps"),
        timestamp: Utc::now(),
        level: "DEBUG".to_string(),
        module: "test".to_string(),
        message: "DEBUG caps".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Debug);
}

#[test]
fn test_log_row_level_case_insensitive_trace() {
    let row = LogRow {
        id: LogId::new("log-trace-caps"),
        timestamp: Utc::now(),
        level: "TRACE".to_string(),
        module: "test".to_string(),
        message: "TRACE caps".to_string(),
        metadata: None,
        user_id: None,
        session_id: None,
        task_id: None,
        trace_id: None,
        context_id: None,
        client_id: None,
    };

    let entry: LogEntry = row.into();
    assert_eq!(entry.level, LogLevel::Trace);
}
