//! Unit tests for LogEntry struct

use systemprompt_logging::{LogEntry, LogLevel};

// ============================================================================
// LogEntry Creation Tests
// ============================================================================

#[test]
fn test_log_entry_new() {
    let entry = LogEntry::new(LogLevel::Info, "test_module", "Test message");

    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(entry.module, "test_module");
    assert_eq!(entry.message, "Test message");
    assert!(entry.metadata.is_none());
}

#[test]
fn test_log_entry_new_with_different_levels() {
    let error = LogEntry::new(LogLevel::Error, "module", "Error message");
    let warn = LogEntry::new(LogLevel::Warn, "module", "Warn message");
    let info = LogEntry::new(LogLevel::Info, "module", "Info message");
    let debug = LogEntry::new(LogLevel::Debug, "module", "Debug message");
    let trace = LogEntry::new(LogLevel::Trace, "module", "Trace message");

    assert_eq!(error.level, LogLevel::Error);
    assert_eq!(warn.level, LogLevel::Warn);
    assert_eq!(info.level, LogLevel::Info);
    assert_eq!(debug.level, LogLevel::Debug);
    assert_eq!(trace.level, LogLevel::Trace);
}

#[test]
fn test_log_entry_has_generated_id() {
    let entry1 = LogEntry::new(LogLevel::Info, "module", "message");
    let entry2 = LogEntry::new(LogLevel::Info, "module", "message");

    // IDs should be unique
    assert_ne!(entry1.id.as_str(), entry2.id.as_str());
}

#[test]
fn test_log_entry_has_timestamp() {
    let before = chrono::Utc::now();
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    let after = chrono::Utc::now();

    assert!(entry.timestamp >= before);
    assert!(entry.timestamp <= after);
}

// ============================================================================
// LogEntry Builder Pattern Tests
// ============================================================================

#[test]
fn test_log_entry_with_metadata() {
    let metadata = serde_json::json!({"key": "value"});
    let entry = LogEntry::new(LogLevel::Info, "module", "message").with_metadata(metadata.clone());

    assert_eq!(entry.metadata, Some(metadata));
}

#[test]
fn test_log_entry_with_user_id() {
    let user_id = systemprompt_identifiers::UserId::new("test-user-123");
    let entry = LogEntry::new(LogLevel::Info, "module", "message").with_user_id(user_id.clone());

    assert_eq!(entry.user_id, user_id);
}

#[test]
fn test_log_entry_with_session_id() {
    let session_id = systemprompt_identifiers::SessionId::new("test-session-456");
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_session_id(session_id.clone());

    assert_eq!(entry.session_id, session_id);
}

#[test]
fn test_log_entry_with_task_id() {
    let task_id = systemprompt_identifiers::TaskId::new("test-task-789");
    let entry = LogEntry::new(LogLevel::Info, "module", "message").with_task_id(task_id.clone());

    assert_eq!(entry.task_id, Some(task_id));
}

#[test]
fn test_log_entry_with_trace_id() {
    let trace_id = systemprompt_identifiers::TraceId::new("test-trace-abc");
    let entry = LogEntry::new(LogLevel::Info, "module", "message").with_trace_id(trace_id.clone());

    assert_eq!(entry.trace_id, trace_id);
}

#[test]
fn test_log_entry_with_context_id() {
    let context_id = systemprompt_identifiers::ContextId::new("test-context-def");
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_context_id(context_id.clone());

    assert_eq!(entry.context_id, Some(context_id));
}

#[test]
fn test_log_entry_with_client_id() {
    let client_id = systemprompt_identifiers::ClientId::new("test-client-ghi");
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_client_id(client_id.clone());

    assert_eq!(entry.client_id, Some(client_id));
}

#[test]
fn test_log_entry_builder_chaining() {
    let user_id = systemprompt_identifiers::UserId::new("chain-user");
    let session_id = systemprompt_identifiers::SessionId::new("chain-session");
    let metadata = serde_json::json!({"action": "test"});

    let entry = LogEntry::new(LogLevel::Error, "auth", "Login failed")
        .with_user_id(user_id.clone())
        .with_session_id(session_id.clone())
        .with_metadata(metadata.clone());

    assert_eq!(entry.level, LogLevel::Error);
    assert_eq!(entry.module, "auth");
    assert_eq!(entry.message, "Login failed");
    assert_eq!(entry.user_id, user_id);
    assert_eq!(entry.session_id, session_id);
    assert_eq!(entry.metadata, Some(metadata));
}

// ============================================================================
// LogEntry Validation Tests
// ============================================================================

#[test]
fn test_log_entry_validate_valid() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert!(entry.validate().is_ok());
}

#[test]
fn test_log_entry_validate_empty_module() {
    let entry = LogEntry::new(LogLevel::Info, "", "message");
    assert!(entry.validate().is_err());
}

#[test]
fn test_log_entry_validate_empty_message() {
    let entry = LogEntry::new(LogLevel::Info, "module", "");
    assert!(entry.validate().is_err());
}

#[test]
fn test_log_entry_validate_with_object_metadata() {
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_metadata(serde_json::json!({}));
    assert!(entry.validate().is_ok());
}

#[test]
fn test_log_entry_validate_with_array_metadata() {
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_metadata(serde_json::json!([]));
    assert!(entry.validate().is_ok());
}

#[test]
fn test_log_entry_validate_with_string_metadata() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message")
        .with_metadata(serde_json::json!("string"));
    assert!(entry.validate().is_ok());
}

#[test]
fn test_log_entry_validate_with_null_metadata() {
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(null));
    assert!(entry.validate().is_ok());
}

#[test]
fn test_log_entry_validate_with_number_metadata() {
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(42));
    assert!(entry.validate().is_err());
}

#[test]
fn test_log_entry_validate_with_boolean_metadata() {
    let entry =
        LogEntry::new(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(true));
    assert!(entry.validate().is_err());
}

// ============================================================================
// LogEntry Display Tests
// ============================================================================

#[test]
fn test_log_entry_display_without_metadata() {
    let entry = LogEntry::new(LogLevel::Info, "test_module", "Test message");
    let display = entry.to_string();

    assert!(display.contains("[INFO ]"));
    assert!(display.contains("test_module"));
    assert!(display.contains("Test message"));
}

#[test]
fn test_log_entry_display_with_metadata() {
    let entry = LogEntry::new(LogLevel::Error, "module", "message")
        .with_metadata(serde_json::json!({"key": "value"}));
    let display = entry.to_string();

    assert!(display.contains("[ERROR]"));
    assert!(display.contains("module"));
    assert!(display.contains("message"));
    assert!(display.contains("key"));
    assert!(display.contains("value"));
}

#[test]
fn test_log_entry_display_all_levels() {
    assert!(LogEntry::new(LogLevel::Error, "m", "msg")
        .to_string()
        .contains("[ERROR]"));
    assert!(LogEntry::new(LogLevel::Warn, "m", "msg")
        .to_string()
        .contains("[WARN ]"));
    assert!(LogEntry::new(LogLevel::Info, "m", "msg")
        .to_string()
        .contains("[INFO ]"));
    assert!(LogEntry::new(LogLevel::Debug, "m", "msg")
        .to_string()
        .contains("[DEBUG]"));
    assert!(LogEntry::new(LogLevel::Trace, "m", "msg")
        .to_string()
        .contains("[TRACE]"));
}

// ============================================================================
// LogEntry Clone and Debug Tests
// ============================================================================

#[test]
fn test_log_entry_clone() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message")
        .with_metadata(serde_json::json!({"key": "value"}));
    let cloned = entry.clone();

    assert_eq!(entry.id.as_str(), cloned.id.as_str());
    assert_eq!(entry.level, cloned.level);
    assert_eq!(entry.module, cloned.module);
    assert_eq!(entry.message, cloned.message);
    assert_eq!(entry.metadata, cloned.metadata);
}

#[test]
fn test_log_entry_debug() {
    let entry = LogEntry::new(LogLevel::Info, "test_module", "Test message");
    let debug = format!("{:?}", entry);

    assert!(debug.contains("LogEntry"));
    assert!(debug.contains("test_module"));
    assert!(debug.contains("Test message"));
}

// ============================================================================
// LogEntry Serialization Tests
// ============================================================================

#[test]
fn test_log_entry_serialize() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    let json = serde_json::to_string(&entry).unwrap();

    assert!(json.contains("\"level\":\"INFO\""));
    assert!(json.contains("\"module\":\"module\""));
    assert!(json.contains("\"message\":\"message\""));
}

#[test]
fn test_log_entry_deserialize() {
    let entry = LogEntry::new(LogLevel::Error, "test", "test message");
    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: LogEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.level, LogLevel::Error);
    assert_eq!(deserialized.module, "test");
    assert_eq!(deserialized.message, "test message");
}

#[test]
fn test_log_entry_roundtrip() {
    let entry = LogEntry::new(LogLevel::Warn, "auth::login", "Authentication failed")
        .with_metadata(serde_json::json!({"attempt": 3, "ip": "192.168.1.1"}));

    let json = serde_json::to_string(&entry).unwrap();
    let parsed: LogEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.level, entry.level);
    assert_eq!(parsed.module, entry.module);
    assert_eq!(parsed.message, entry.message);
    assert_eq!(parsed.metadata, entry.metadata);
}

// ============================================================================
// LogEntry Default IDs Tests
// ============================================================================

#[test]
fn test_log_entry_default_user_id_is_system() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert_eq!(entry.user_id, systemprompt_identifiers::UserId::system());
}

#[test]
fn test_log_entry_default_session_id_is_system() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert_eq!(
        entry.session_id,
        systemprompt_identifiers::SessionId::system()
    );
}

#[test]
fn test_log_entry_default_trace_id_is_system() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert_eq!(entry.trace_id, systemprompt_identifiers::TraceId::system());
}

#[test]
fn test_log_entry_default_task_id_is_none() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert!(entry.task_id.is_none());
}

#[test]
fn test_log_entry_default_context_id_is_none() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert!(entry.context_id.is_none());
}

#[test]
fn test_log_entry_default_client_id_is_none() {
    let entry = LogEntry::new(LogLevel::Info, "module", "message");
    assert!(entry.client_id.is_none());
}
