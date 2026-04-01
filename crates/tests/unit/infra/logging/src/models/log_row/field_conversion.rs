//! Unit tests for LogRow to LogEntry field conversion (metadata, IDs, timestamp)

use chrono::Utc;
use systemprompt_identifiers::LogId;
use systemprompt_logging::models::LogRow;
use systemprompt_logging::{LogEntry, LogLevel};

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
