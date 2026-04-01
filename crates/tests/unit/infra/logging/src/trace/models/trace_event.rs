//! Unit tests for TraceEvent struct

use chrono::Utc;
use systemprompt_logging::TraceEvent;

#[test]
fn test_trace_event_creation() {
    let event = TraceEvent {
        event_type: "test_event".to_string(),
        timestamp: Utc::now(),
        details: "Test details".to_string(),
        user_id: Some("user-123".to_string().into()),
        session_id: Some("session-456".to_string().into()),
        task_id: Some("task-789".to_string().into()),
        context_id: Some("context-abc".to_string().into()),
        metadata: Some(r#"{"key": "value"}"#.to_string()),
    };

    assert_eq!(event.event_type, "test_event");
    assert_eq!(event.details, "Test details");
    assert_eq!(event.user_id, Some("user-123".to_string().into()));
    assert_eq!(event.session_id, Some("session-456".to_string().into()));
    assert_eq!(event.task_id, Some("task-789".to_string().into()));
    assert_eq!(event.context_id, Some("context-abc".to_string().into()));
    assert!(event.metadata.is_some());
}

#[test]
fn test_trace_event_minimal() {
    let event = TraceEvent {
        event_type: "minimal".to_string(),
        timestamp: Utc::now(),
        details: String::new(),
        user_id: None,
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    };

    assert_eq!(event.event_type, "minimal");
    assert!(event.user_id.is_none());
    assert!(event.session_id.is_none());
    assert!(event.task_id.is_none());
    assert!(event.context_id.is_none());
    assert!(event.metadata.is_none());
}

#[test]
fn test_trace_event_clone() {
    let event = TraceEvent {
        event_type: "clone_test".to_string(),
        timestamp: Utc::now(),
        details: "Clone details".to_string(),
        user_id: Some("user".to_string().into()),
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    };

    let cloned = event.clone();
    assert_eq!(event.event_type, cloned.event_type);
    assert_eq!(event.details, cloned.details);
    assert_eq!(event.user_id, cloned.user_id);
}

#[test]
fn test_trace_event_serialize() {
    let event = TraceEvent {
        event_type: "serialize_test".to_string(),
        timestamp: Utc::now(),
        details: "Serialize details".to_string(),
        user_id: None,
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("serialize_test"));
    assert!(json.contains("Serialize details"));
}

#[test]
fn test_trace_event_deserialize() {
    let json = r#"{
        "event_type": "deserialized",
        "timestamp": "2024-01-01T00:00:00Z",
        "details": "Deserialized details",
        "user_id": null,
        "session_id": null,
        "task_id": null,
        "context_id": null,
        "metadata": null
    }"#;

    let event: TraceEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type, "deserialized");
    assert_eq!(event.details, "Deserialized details");
}

#[test]
fn test_trace_event_roundtrip() {
    let event = TraceEvent {
        event_type: "roundtrip".to_string(),
        timestamp: Utc::now(),
        details: "Roundtrip test".to_string(),
        user_id: Some("user".to_string().into()),
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: TraceEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(event.event_type, deserialized.event_type);
    assert_eq!(event.details, deserialized.details);
    assert_eq!(event.user_id, deserialized.user_id);
}
