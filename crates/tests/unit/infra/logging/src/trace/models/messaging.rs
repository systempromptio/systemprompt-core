//! Tests for ConversationMessage, ToolLogEntry, TaskArtifact, and roundtrip serialization

use chrono::Utc;
use serde_json::json;
use systemprompt_logging::{
    ConversationMessage, TaskArtifact, ToolLogEntry,
};

// ============================================================================
// ConversationMessage Tests
// ============================================================================

#[test]
fn test_conversation_message_creation() {
    let msg = ConversationMessage {
        role: "user".to_string(),
        content: "Hello, how can you help me?".to_string(),
        sequence_number: 1,
    };

    assert_eq!(msg.role, "user");
    assert_eq!(msg.content, "Hello, how can you help me?");
    assert_eq!(msg.sequence_number, 1);
}

#[test]
fn test_conversation_message_assistant_role() {
    let msg = ConversationMessage {
        role: "assistant".to_string(),
        content: "I can help you with many tasks.".to_string(),
        sequence_number: 2,
    };

    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.sequence_number, 2);
}

#[test]
fn test_conversation_message_system_role() {
    let msg = ConversationMessage {
        role: "system".to_string(),
        content: "You are a helpful assistant.".to_string(),
        sequence_number: 0,
    };

    assert_eq!(msg.role, "system");
    assert_eq!(msg.sequence_number, 0);
}

#[test]
fn test_conversation_message_clone() {
    let msg = ConversationMessage {
        role: "user".to_string(),
        content: "Clone test".to_string(),
        sequence_number: 5,
    };

    let cloned = msg.clone();
    assert_eq!(msg.role, cloned.role);
    assert_eq!(msg.content, cloned.content);
    assert_eq!(msg.sequence_number, cloned.sequence_number);
}

#[test]
fn test_conversation_message_serialize() {
    let msg = ConversationMessage {
        role: "user".to_string(),
        content: "Serialize test".to_string(),
        sequence_number: 1,
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("role"));
    assert!(json.contains("content"));
    assert!(json.contains("sequence_number"));
}

// ============================================================================
// ToolLogEntry Tests
// ============================================================================

#[test]
fn test_tool_log_entry_creation() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "mcp::tool".to_string(),
        message: "Tool executed successfully".to_string(),
    };

    assert_eq!(entry.level, "info");
    assert_eq!(entry.module, "mcp::tool");
    assert!(entry.message.contains("successfully"));
}

#[test]
fn test_tool_log_entry_error_level() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "error".to_string(),
        module: "mcp::server".to_string(),
        message: "Server connection failed".to_string(),
    };

    assert_eq!(entry.level, "error");
}

#[test]
fn test_tool_log_entry_clone() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "warn".to_string(),
        module: "clone".to_string(),
        message: "Clone test".to_string(),
    };

    let cloned = entry.clone();
    assert_eq!(entry.level, cloned.level);
    assert_eq!(entry.module, cloned.module);
    assert_eq!(entry.message, cloned.message);
}

#[test]
fn test_tool_log_entry_serialize() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "info".to_string(),
        module: "test".to_string(),
        message: "Serialize test".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("level"));
    assert!(json.contains("module"));
    assert!(json.contains("message"));
}

// ============================================================================
// TaskArtifact Tests
// ============================================================================

#[test]
fn test_task_artifact_creation() {
    let artifact = TaskArtifact {
        artifact_id: "art-123".to_string().into(),
        artifact_type: "file".to_string(),
        name: Some("output.txt".to_string()),
        source: Some("tool_execution".to_string()),
        tool_name: Some("file_writer".to_string()),
        part_kind: Some("text".to_string()),
        text_content: Some("File contents".to_string()),
        data_content: None,
    };

    assert_eq!(artifact.artifact_id, "art-123");
    assert_eq!(artifact.artifact_type, "file");
    assert_eq!(artifact.name, Some("output.txt".to_string()));
    artifact.text_content.expect("artifact.text_content should be present");
}

#[test]
fn test_task_artifact_with_data_content() {
    let artifact = TaskArtifact {
        artifact_id: "art-data".to_string().into(),
        artifact_type: "json".to_string(),
        name: Some("data.json".to_string()),
        source: None,
        tool_name: None,
        part_kind: Some("data".to_string()),
        text_content: None,
        data_content: Some(json!({"key": "value", "count": 42})),
    };

    artifact.data_content.as_ref().expect("artifact.data_content should be present");
    let data = artifact.data_content.as_ref().unwrap();
    assert_eq!(data["key"], "value");
    assert_eq!(data["count"], 42);
}

#[test]
fn test_task_artifact_minimal() {
    let artifact = TaskArtifact {
        artifact_id: "art-min".to_string().into(),
        artifact_type: "unknown".to_string(),
        name: None,
        source: None,
        tool_name: None,
        part_kind: None,
        text_content: None,
        data_content: None,
    };

    assert!(artifact.name.is_none());
    assert!(artifact.source.is_none());
    assert!(artifact.tool_name.is_none());
    assert!(artifact.text_content.is_none());
    assert!(artifact.data_content.is_none());
}

#[test]
fn test_task_artifact_clone() {
    let artifact = TaskArtifact {
        artifact_id: "clone".to_string().into(),
        artifact_type: "file".to_string(),
        name: Some("test.txt".to_string()),
        source: Some("user".to_string()),
        tool_name: None,
        part_kind: None,
        text_content: Some("content".to_string()),
        data_content: None,
    };

    let cloned = artifact.clone();
    assert_eq!(artifact.artifact_id, cloned.artifact_id);
    assert_eq!(artifact.name, cloned.name);
    assert_eq!(artifact.text_content, cloned.text_content);
}

#[test]
fn test_task_artifact_serialize() {
    let artifact = TaskArtifact {
        artifact_id: "ser".to_string().into(),
        artifact_type: "text".to_string(),
        name: None,
        source: None,
        tool_name: None,
        part_kind: None,
        text_content: Some("Serialized".to_string()),
        data_content: None,
    };

    let json = serde_json::to_string(&artifact).unwrap();
    assert!(json.contains("artifact_id"));
    assert!(json.contains("artifact_type"));
    assert!(json.contains("Serialized"));
}

// ============================================================================
// Roundtrip Serialization Tests
// ============================================================================

#[test]
fn test_conversation_message_roundtrip() {
    let msg = ConversationMessage {
        role: "user".to_string(),
        content: "Roundtrip test".to_string(),
        sequence_number: 42,
    };

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ConversationMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.role, "user");
    assert_eq!(parsed.content, "Roundtrip test");
    assert_eq!(parsed.sequence_number, 42);
}

#[test]
fn test_tool_log_entry_roundtrip() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "warn".to_string(),
        module: "roundtrip::test".to_string(),
        message: "Roundtrip entry".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    let parsed: ToolLogEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.level, "warn");
    assert_eq!(parsed.module, "roundtrip::test");
    assert_eq!(parsed.message, "Roundtrip entry");
    assert_eq!(parsed.timestamp, entry.timestamp);
}

#[test]
fn test_task_artifact_roundtrip_with_data() {
    let artifact = TaskArtifact {
        artifact_id: "rt-art".to_string().into(),
        artifact_type: "json".to_string(),
        name: Some("data.json".to_string()),
        source: Some("test".to_string()),
        tool_name: Some("tool".to_string()),
        part_kind: Some("data".to_string()),
        text_content: None,
        data_content: Some(json!({"nested": {"key": "value"}})),
    };

    let json = serde_json::to_string(&artifact).unwrap();
    let parsed: TaskArtifact = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.artifact_id, "rt-art");
    assert_eq!(parsed.name, Some("data.json".to_string()));
    let data = parsed.data_content.expect("data_content should roundtrip");
    assert_eq!(data["nested"]["key"], "value");
}

#[test]
fn test_task_artifact_roundtrip_minimal() {
    let artifact = TaskArtifact {
        artifact_id: "rt-min".to_string().into(),
        artifact_type: "unknown".to_string(),
        name: None,
        source: None,
        tool_name: None,
        part_kind: None,
        text_content: None,
        data_content: None,
    };

    let json = serde_json::to_string(&artifact).unwrap();
    let parsed: TaskArtifact = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.artifact_id, "rt-min");
    assert!(parsed.name.is_none());
    assert!(parsed.text_content.is_none());
    assert!(parsed.data_content.is_none());
}
