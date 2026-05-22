//! Unit tests for context models
//!
//! Tests cover:
//! - ContextMessage serialization and deserialization
//! - ContextDetail structure
//! - ContextStateEvent variants and methods

use chrono::Utc;
use systemprompt_agent::models::a2a::{Part, TextPart};
use systemprompt_agent::models::context::{ContextDetail, ContextMessage, ContextStateEvent};
use systemprompt_identifiers::{ContextId, McpExecutionId, MessageId};
use systemprompt_models::UserContext;
use systemprompt_test_fixtures::fixture_user_id;

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";
const TEST_CONTEXT_ID_B: &str = "00000000-0000-4000-8000-000000000002";
const TEST_CONTEXT_ID_C: &str = "00000000-0000-4000-8000-000000000003";
const TEST_CONTEXT_ID_D: &str = "00000000-0000-4000-8000-000000000004";
const TEST_CONTEXT_ID_E: &str = "00000000-0000-4000-8000-000000000005";
const TEST_CONTEXT_ID_F: &str = "00000000-0000-4000-8000-000000000006";
const TEST_CONTEXT_ID_G: &str = "00000000-0000-4000-8000-000000000007";

#[test]
fn test_context_message_serialize() {
    let message = ContextMessage {
        message_id: MessageId::new("msg-123"),
        role: "user".to_string(),
        created_at: Utc::now(),
        sequence_number: 1,
        parts: vec![Part::Text(TextPart {
            text: "Hello".to_string(),
        })],
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
        "parts": [{"text": "Response"}]
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
        message_id: MessageId::new("msg-debug"),
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
        message_id: MessageId::new("msg-clone"),
        role: "system".to_string(),
        created_at: Utc::now(),
        sequence_number: 5,
        parts: vec![Part::Text(TextPart {
            text: "test".to_string(),
        })],
    };

    let cloned = message.clone();
    assert_eq!(cloned.message_id.as_str(), message.message_id.as_str());
    assert_eq!(cloned.role, message.role);
    assert_eq!(cloned.sequence_number, message.sequence_number);
}

#[test]
fn test_context_detail_serialize() {
    let detail = ContextDetail {
        context: UserContext {
            context_id: ContextId::new(TEST_CONTEXT_ID_A),
            name: "Test Context".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: fixture_user_id(),
        },
        messages: vec![],
    };

    let json = serde_json::to_string(&detail).unwrap();
    assert!(json.contains(TEST_CONTEXT_ID_A));
    assert!(json.contains("Test Context"));
}

#[test]
fn test_context_detail_with_messages() {
    let detail = ContextDetail {
        context: UserContext {
            context_id: ContextId::new(TEST_CONTEXT_ID_B),
            name: "Conversation".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: fixture_user_id(),
        },
        messages: vec![
            ContextMessage {
                message_id: MessageId::new("msg-1"),
                role: "user".to_string(),
                created_at: Utc::now(),
                sequence_number: 1,
                parts: vec![Part::Text(TextPart {
                    text: "Hello".to_string(),
                })],
            },
            ContextMessage {
                message_id: MessageId::new("msg-2"),
                role: "assistant".to_string(),
                created_at: Utc::now(),
                sequence_number: 2,
                parts: vec![Part::Text(TextPart {
                    text: "Hi there".to_string(),
                })],
            },
        ],
    };

    assert_eq!(detail.messages.len(), 2);
    assert_eq!(detail.messages[0].sequence_number, 1);
    assert_eq!(detail.messages[1].sequence_number, 2);
}

#[test]
fn test_context_state_event_tool_execution_completed_context_id() {
    let event = ContextStateEvent::ToolExecutionCompleted {
        context_id: ContextId::new(TEST_CONTEXT_ID_C),
        execution_id: McpExecutionId::new("exec-123"),
        tool_name: "search".to_string(),
        server_name: "brave".to_string(),
        output: Some("Results".to_string()),
        artifact: None,
        status: "success".to_string(),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_C));
}

#[test]
fn test_context_state_event_task_status_changed_context_id() {
    let event = ContextStateEvent::TaskStatusChanged {
        task: systemprompt_agent::Task::default(),
        context_id: ContextId::new(TEST_CONTEXT_ID_D),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_D));
}

#[test]
fn test_context_state_event_context_created() {
    let event = ContextStateEvent::ContextCreated {
        context_id: ContextId::new(TEST_CONTEXT_ID_E),
        context: UserContext {
            context_id: ContextId::new(TEST_CONTEXT_ID_E),
            name: "New Context".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: fixture_user_id(),
        },
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_E));
}

#[test]
fn test_context_state_event_context_updated() {
    let event = ContextStateEvent::ContextUpdated {
        context_id: ContextId::new(TEST_CONTEXT_ID_F),
        name: "Updated Name".to_string(),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_F));
}

#[test]
fn test_context_state_event_context_deleted() {
    let event = ContextStateEvent::ContextDeleted {
        context_id: ContextId::new(TEST_CONTEXT_ID_G),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_G));
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
        context_id: ContextId::new(TEST_CONTEXT_ID_A),
        agent_name: Some("test-agent".to_string()),
        timestamp: Utc::now(),
    };

    assert_eq!(event.context_id(), Some(TEST_CONTEXT_ID_A));
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
        context_id: ContextId::new(TEST_CONTEXT_ID_B),
        name: "Serialized".to_string(),
        timestamp: Utc::now(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("context_updated"));
    assert!(json.contains(TEST_CONTEXT_ID_B));
    assert!(json.contains("Serialized"));
}
