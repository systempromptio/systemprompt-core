//! Unit tests for LogRow creation and level conversion

use chrono::Utc;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};
use systemprompt_logging::models::LogRow;
use systemprompt_logging::{LogEntry, LogLevel};

#[test]
fn test_log_row_creation() {
    let row = LogRow {
        id: LogId::new("log-test-123"),
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test::module".to_string(),
        message: "Test message".to_string(),
        metadata: Some(r#"{"key": "value"}"#.to_string()),
        user_id: Some(UserId::new("user-123")),
        session_id: Some(SessionId::new("session-456")),
        task_id: Some(TaskId::new("task-789")),
        trace_id: Some(TraceId::new("trace-abc")),
        context_id: Some(ContextId::new("00000000-0000-4000-8000-00000000def0")),
        client_id: Some(ClientId::new("client-ghi")),
    };

    assert_eq!(row.level, "info");
    assert_eq!(row.module, "test::module");
    assert_eq!(row.message, "Test message");
    row.metadata.as_ref().expect("metadata should be set");
    row.user_id.as_ref().expect("user_id should be set");
    row.session_id.as_ref().expect("session_id should be set");
    row.task_id.as_ref().expect("task_id should be set");
    row.trace_id.as_ref().expect("trace_id should be set");
    row.context_id.as_ref().expect("context_id should be set");
    row.client_id.as_ref().expect("client_id should be set");
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
