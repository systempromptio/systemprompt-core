//! Unit tests for context models
//!
//! Tests cover:
//! - ContextMessage serialization and deserialization
//! - ContextDetail structure
//! - ContextStateEvent variants and methods

use chrono::Utc;
use systemprompt_agent::models::context::{ContextDetail, ContextMessage, ContextStateEvent};
use systemprompt_identifiers::{ContextId, McpExecutionId, MessageId, UserId};
use systemprompt_models::UserContext;

// ============================================================================
// ContextMessage Tests
// ============================================================================

#[test]
fn test_context_message_serialize() {
    let message = ContextMessage {
        message_id: MessageId::from("msg-123"),
        role: "user".to_string(),
        created_at: Utc::now(),
        sequence_number: 1,
        parts: vec![serde_json::json!({"type": "text", "text": "Hello"})],
    };

    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("msg-123"));
    assert!(json.contains("user"));
    assert!(json.contains("Hello"));
}

#[test]
fn test_context_message_deserialize() {
    let json = r#"{
        "message_id": "msg-456",
        "role": "assistant",
        "created_at": "2024-01-01T00:00:00Z",
        "sequence_number": 2,
        "parts": [{"type": "text", "content": "Response"}]
    }"#;

    let message: ContextMessage = serde_json::from_str(json).unwrap();
    assert_eq!(message.message_id.as_str(), "msg-456");
    assert_eq!(message.role, "assistant");
    assert_eq!(message.sequence_number, 2);
    assert_eq!(message.parts.len(), 1);
}

#[test]
fn test_context_message_debug() {
    let message = ContextMessage {
        message_id: MessageId::from("msg-debug"),
        role: "user".to_string(),
        created_at: Utc::now(),
        sequence_number: 0,
        parts: vec![],
    };

    let debug_str = format!("{:?}", message);
    assert!(debug_str.contains("ContextMessage"));
    assert!(debug_str.contains("msg-debug"));
}

#[test]
fn test_context_message_clone() {
    let message = ContextMessage {
        message_id: MessageId::from("msg-clone"),
        role: "system".to_string(),
        created_at: Utc::now(),
        sequence_number: 5,
        parts: vec![serde_json::json!({"test": true})],
    };

    let cloned = message.clone();
    assert_eq!(cloned.message_id.as_str(), message.message_id.as_str());
    assert_eq!(cloned.role, message.role);
    assert_eq!(cloned.sequence_number, message.sequence_number);
}

// ============================================================================
// ContextDetail Tests
// ============================================================================

#[test]
fn test_context_detail_serialize() {
    let detail = ContextDetail {
        context: UserContext {
            context_id: ContextId::new("ctx-123"),
            name: "Test Context".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: UserId::new("user-1"),
        },
        messages: vec![],
    };

    let json = serde_json::to_string(&detail).unwrap();
    assert!(json.contains("ctx-123"));
    assert!(json.contains("Test Context"));
}

#[test]
fn test_context_detail_with_messages() {
    let detail = ContextDetail {
        context: UserContext {
            context_id: ContextId::new("ctx-456"),
            name: "Conversation".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: UserId::new("user-2"),
        },
        messages: vec![
            ContextMessage {
                message_id: MessageId::from("msg-1"),
                role: "user".to_string(),
                created_at: Utc::now(),
                sequence_number: 1,
                parts: vec![serde_json::json!({"text": "Hello"})],
            },
            ContextMessage {
                message_id: MessageId::from("msg-2"),
                role: "assistant".to_string(),
                created_at: Utc::now(),
                sequence_number: 2,
                parts: vec![serde_json::json!({"text": "Hi there"})],
            },
        ],
    };

    assert_eq!(detail.messages.len(), 2);
    assert_eq!(detail.messages[0].sequence_number, 1);
    assert_eq!(detail.messages[1].sequence_number, 2);
}

// ============================================================================
// ContextStateEvent Tests
// ============================================================================

#[test]
fn test_context_state_event_tool_execution_completed_context_id() {
    let event = ContextStateEvent::ToolExecutionCompleted {
        context_id: ContextId::from("ctx-tool-exec"),
        execution_id: McpExecutionId::from("exec-123"),
        tool_name: "search".to_string(),
        server_name: "brave".to_string(),
        output: Some("Results".to_string()),
        artifact: None,
        status: "success".to_string(),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-tool-exec"));
}

#[test]
fn test_context_state_event_task_status_changed_context_id() {
    let event = ContextStateEvent::TaskStatusChanged {
        task: systemprompt_agent::Task::default(),
        context_id: ContextId::from("ctx-task-status"),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-task-status"));
}

#[test]
fn test_context_state_event_context_created() {
    let event = ContextStateEvent::ContextCreated {
        context_id: ContextId::from("ctx-new"),
        context: UserContext {
            context_id: ContextId::new("ctx-new"),
            name: "New Context".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: UserId::new("user-1"),
        },
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-new"));
}

#[test]
fn test_context_state_event_context_updated() {
    let event = ContextStateEvent::ContextUpdated {
        context_id: ContextId::from("ctx-updated"),
        name: "Updated Name".to_string(),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-updated"));
}

#[test]
fn test_context_state_event_context_deleted() {
    let event = ContextStateEvent::ContextDeleted {
        context_id: ContextId::from("ctx-deleted"),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-deleted"));
}

#[test]
fn test_context_state_event_heartbeat_no_context_id() {
    let event = ContextStateEvent::Heartbeat {
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), None);
}

#[test]
fn test_context_state_event_current_agent() {
    let event = ContextStateEvent::CurrentAgent {
        context_id: ContextId::from("ctx-current-agent"),
        agent_name: Some("test-agent".to_string()),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some("ctx-current-agent"));
}

#[test]
fn test_context_state_event_timestamp() {
    let now = Utc::now();

    let event = ContextStateEvent::Heartbeat { timestamp: now };

    assert_eq!(event.timestamp(), now);
}

#[test]
fn test_context_state_event_serialize() {
    let event = ContextStateEvent::ContextUpdated {
        context_id: ContextId::from("ctx-serialize"),
        name: "Serialized".to_string(),
        timestamp: Utc::now(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("context_updated"));
    assert!(json.contains("ctx-serialize"));
    assert!(json.contains("Serialized"));
}

#[test]
fn test_context_state_event_deserialize() {
    let json = r#"{
        "type": "context_deleted",
        "context_id": "ctx-deserialize",
        "timestamp": "2024-01-01T12:00:00Z"
    }"#;

    let event: ContextStateEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.context_id(), Some("ctx-deserialize"));
}
