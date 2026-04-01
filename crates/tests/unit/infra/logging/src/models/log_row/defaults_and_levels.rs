//! Unit tests for LogRow default ID conversion and level string case variations

use chrono::Utc;
use systemprompt_identifiers::LogId;
use systemprompt_logging::models::LogRow;
use systemprompt_logging::{LogEntry, LogLevel};

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
