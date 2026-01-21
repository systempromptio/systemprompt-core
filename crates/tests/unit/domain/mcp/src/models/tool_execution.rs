//! Unit tests for ToolExecution model

use chrono::Utc;
use systemprompt_mcp::models::ToolExecution;
use systemprompt_identifiers::{AiToolCallId, ContextId, McpExecutionId, UserId};

fn create_test_execution() -> ToolExecution {
    ToolExecution {
        mcp_execution_id: McpExecutionId::new("exec-123".to_string()),
        tool_name: "test-tool".to_string(),
        server_name: "test-server".to_string(),
        context_id: Some(ContextId::new("ctx-456".to_string())),
        ai_tool_call_id: Some(AiToolCallId::new("call-789".to_string())),
        user_id: UserId::new("user-abc".to_string()),
        status: "success".to_string(),
        input: r#"{"query": "test"}"#.to_string(),
        output: Some(r#"{"result": "ok"}"#.to_string()),
        error_message: None,
        execution_time_ms: Some(150),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
    }
}

// ============================================================================
// ToolExecution Field Access Tests
// ============================================================================

#[test]
fn test_tool_execution_fields() {
    let exec = create_test_execution();

    assert_eq!(exec.tool_name, "test-tool");
    assert_eq!(exec.server_name, "test-server");
    assert_eq!(exec.status, "success");
    assert!(exec.context_id.is_some());
    assert!(exec.ai_tool_call_id.is_some());
    assert_eq!(exec.execution_time_ms, Some(150));
}

#[test]
fn test_tool_execution_with_error() {
    let mut exec = create_test_execution();
    exec.status = "failed".to_string();
    exec.error_message = Some("Connection timeout".to_string());
    exec.output = None;
    exec.completed_at = None;

    assert_eq!(exec.status, "failed");
    assert_eq!(exec.error_message, Some("Connection timeout".to_string()));
    assert!(exec.output.is_none());
    assert!(exec.completed_at.is_none());
}

#[test]
fn test_tool_execution_pending() {
    let mut exec = create_test_execution();
    exec.status = "pending".to_string();
    exec.output = None;
    exec.execution_time_ms = None;
    exec.completed_at = None;

    assert_eq!(exec.status, "pending");
    assert!(exec.output.is_none());
    assert!(exec.execution_time_ms.is_none());
    assert!(exec.completed_at.is_none());
}

#[test]
fn test_tool_execution_without_context() {
    let mut exec = create_test_execution();
    exec.context_id = None;
    exec.ai_tool_call_id = None;

    assert!(exec.context_id.is_none());
    assert!(exec.ai_tool_call_id.is_none());
}

// ============================================================================
// ToolExecution Clone Tests
// ============================================================================

#[test]
fn test_tool_execution_clone() {
    let exec = create_test_execution();
    let cloned = exec.clone();

    assert_eq!(exec.tool_name, cloned.tool_name);
    assert_eq!(exec.server_name, cloned.server_name);
    assert_eq!(exec.status, cloned.status);
    assert_eq!(exec.input, cloned.input);
    assert_eq!(exec.output, cloned.output);
}

// ============================================================================
// ToolExecution Debug Tests
// ============================================================================

#[test]
fn test_tool_execution_debug() {
    let exec = create_test_execution();
    let debug_str = format!("{:?}", exec);

    assert!(debug_str.contains("ToolExecution"));
    assert!(debug_str.contains("test-tool"));
    assert!(debug_str.contains("test-server"));
    assert!(debug_str.contains("success"));
}

// ============================================================================
// ToolExecution Serialization Tests
// ============================================================================

#[test]
fn test_tool_execution_serialize() {
    let exec = create_test_execution();
    let json = serde_json::to_string(&exec).unwrap();

    assert!(json.contains("test-tool"));
    assert!(json.contains("test-server"));
    assert!(json.contains("success"));
}

#[test]
fn test_tool_execution_deserialize() {
    let exec = create_test_execution();
    let json = serde_json::to_string(&exec).unwrap();
    let deserialized: ToolExecution = serde_json::from_str(&json).unwrap();

    assert_eq!(exec.tool_name, deserialized.tool_name);
    assert_eq!(exec.server_name, deserialized.server_name);
    assert_eq!(exec.status, deserialized.status);
}

#[test]
fn test_tool_execution_roundtrip() {
    let exec = create_test_execution();
    let json = serde_json::to_string(&exec).unwrap();
    let deserialized: ToolExecution = serde_json::from_str(&json).unwrap();
    let json2 = serde_json::to_string(&deserialized).unwrap();

    assert_eq!(json, json2);
}
