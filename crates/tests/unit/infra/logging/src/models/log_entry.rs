//! Unit tests for LogEntry struct

use std::sync::Once;

use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel};
use systemprompt_models::services::SystemAdmin;
use systemprompt_test_fixtures::fixture_user_id;

static INSTALL_ADMIN: Once = Once::new();

fn ensure_system_admin_installed() {
    INSTALL_ADMIN.call_once(|| {
        let _ = SystemAdmin::install(SystemAdmin::new(
            UserId::new("admin"),
            "admin".to_string(),
        ));
    });
}

fn fixture_session_id() -> SessionId {
    SessionId::new("test-session")
}

fn fixture_trace_id() -> TraceId {
    TraceId::new("test-trace")
}

fn make_entry(level: LogLevel, module: &str, message: &str) -> LogEntry {
    LogEntry::new(
        level,
        module,
        message,
        LogActor::new(fixture_user_id(), fixture_session_id(), fixture_trace_id()),
    )
}

#[test]
fn test_log_entry_new() {
    let entry = make_entry(LogLevel::Info, "test_module", "Test message");

    assert_eq!(entry.level, LogLevel::Info);
    assert_eq!(entry.module, "test_module");
    assert_eq!(entry.message, "Test message");
    assert!(entry.metadata.is_none());
}

#[test]
fn test_log_entry_carries_required_attribution() {
    let entry = make_entry(LogLevel::Info, "module", "message");

    assert_eq!(entry.user_id, fixture_user_id());
    assert_eq!(entry.session_id, fixture_session_id());
    assert_eq!(entry.trace_id, fixture_trace_id());
}

#[test]
fn test_log_entry_new_with_different_levels() {
    let error = make_entry(LogLevel::Error, "module", "Error message");
    let warn = make_entry(LogLevel::Warn, "module", "Warn message");
    let info = make_entry(LogLevel::Info, "module", "Info message");
    let debug = make_entry(LogLevel::Debug, "module", "Debug message");
    let trace = make_entry(LogLevel::Trace, "module", "Trace message");

    assert_eq!(error.level, LogLevel::Error);
    assert_eq!(warn.level, LogLevel::Warn);
    assert_eq!(info.level, LogLevel::Info);
    assert_eq!(debug.level, LogLevel::Debug);
    assert_eq!(trace.level, LogLevel::Trace);
}

#[test]
fn test_log_entry_has_generated_id() {
    let entry1 = make_entry(LogLevel::Info, "module", "message");
    let entry2 = make_entry(LogLevel::Info, "module", "message");

    assert_ne!(entry1.id.as_str(), entry2.id.as_str());
}

#[test]
fn test_log_entry_has_timestamp() {
    let before = chrono::Utc::now();
    let entry = make_entry(LogLevel::Info, "module", "message");
    let after = chrono::Utc::now();

    assert!(entry.timestamp >= before);
    assert!(entry.timestamp <= after);
}

#[test]
fn test_log_entry_with_metadata() {
    let metadata = serde_json::json!({"key": "value"});
    let entry = make_entry(LogLevel::Info, "module", "message").with_metadata(metadata.clone());

    assert_eq!(entry.metadata, Some(metadata));
}

#[test]
fn test_log_entry_with_task_id() {
    let task_id = systemprompt_identifiers::TaskId::new("test-task-789");
    let entry = make_entry(LogLevel::Info, "module", "message").with_task_id(task_id.clone());

    assert_eq!(entry.task_id, Some(task_id));
}

#[test]
fn test_log_entry_validate_valid() {
    let entry = make_entry(LogLevel::Info, "module", "message");
    entry.validate().expect("entry.validate() should succeed");
}

#[test]
fn test_log_entry_validate_empty_module() {
    let entry = make_entry(LogLevel::Info, "", "message");
    entry.validate().unwrap_err();
}

#[test]
fn test_log_entry_validate_empty_message() {
    let entry = make_entry(LogLevel::Info, "module", "");
    entry.validate().unwrap_err();
}

#[test]
fn test_log_entry_validate_with_object_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!({}));
    entry.validate().expect("entry.validate() should succeed");
}

#[test]
fn test_log_entry_validate_with_array_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!([]));
    entry.validate().expect("entry.validate() should succeed");
}

#[test]
fn test_log_entry_validate_with_string_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!("string"));
    entry.validate().expect("entry.validate() should succeed");
}

#[test]
fn test_log_entry_validate_with_null_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(null));
    entry.validate().expect("entry.validate() should succeed");
}

#[test]
fn test_log_entry_validate_with_number_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(42));
    entry.validate().unwrap_err();
}

#[test]
fn test_log_entry_validate_with_boolean_metadata() {
    let entry =
        make_entry(LogLevel::Info, "module", "message").with_metadata(serde_json::json!(true));
    entry.validate().unwrap_err();
}

#[test]
fn test_log_entry_display_without_metadata() {
    let entry = make_entry(LogLevel::Info, "test_module", "Test message");
    let display = entry.to_string();

    assert!(display.contains("[INFO ]"));
    assert!(display.contains("test_module"));
    assert!(display.contains("Test message"));
}

#[test]
fn test_log_entry_display_with_metadata() {
    let entry = make_entry(LogLevel::Error, "module", "message")
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
    assert!(
        make_entry(LogLevel::Error, "m", "msg")
            .to_string()
            .contains("[ERROR]")
    );
    assert!(
        make_entry(LogLevel::Warn, "m", "msg")
            .to_string()
            .contains("[WARN ]")
    );
    assert!(
        make_entry(LogLevel::Info, "m", "msg")
            .to_string()
            .contains("[INFO ]")
    );
    assert!(
        make_entry(LogLevel::Debug, "m", "msg")
            .to_string()
            .contains("[DEBUG]")
    );
    assert!(
        make_entry(LogLevel::Trace, "m", "msg")
            .to_string()
            .contains("[TRACE]")
    );
}

#[test]
fn test_log_entry_clone() {
    let entry = make_entry(LogLevel::Info, "module", "message")
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
    let entry = make_entry(LogLevel::Error, "debug_module", "debug_message");
    let debug = format!("{:?}", entry);

    assert!(debug.contains("LogEntry"));
    assert!(debug.contains("Error"));
    assert!(debug.contains("debug_module"));
    assert!(debug.contains("debug_message"));
}

#[test]
fn test_log_entry_serialize() {
    let entry = make_entry(LogLevel::Info, "module", "message");
    let json = serde_json::to_string(&entry).unwrap();

    assert!(json.contains("\"level\":\"INFO\""));
    assert!(json.contains("\"module\":\"module\""));
    assert!(json.contains("\"message\":\"message\""));
}

#[test]
fn test_log_entry_roundtrip() {
    let entry = make_entry(LogLevel::Warn, "auth::login", "Authentication failed")
        .with_metadata(serde_json::json!({"attempt": 3, "ip": "192.168.1.1"}));

    let json = serde_json::to_string(&entry).unwrap();
    let parsed: LogEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.level, entry.level);
    assert_eq!(parsed.module, entry.module);
    assert_eq!(parsed.message, entry.message);
    assert_eq!(parsed.metadata, entry.metadata);
}

#[test]
fn test_log_entry_platform_event_attributes_to_platform_owner() {
    ensure_system_admin_installed();
    let actor = LogActor::platform(TraceId::new("trace-platform"))
        .expect("SystemAdmin installed for the test");
    let entry = LogEntry::new(LogLevel::Info, "module", "platform-internal event", actor);

    assert_eq!(entry.user_id, UserId::new("admin"));
    assert_eq!(entry.session_id, SessionId::system());
    assert_eq!(entry.trace_id.as_str(), "trace-platform");
}
