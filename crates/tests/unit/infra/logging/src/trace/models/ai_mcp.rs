//! Unit tests for AiRequestInfo and McpToolExecution structs

use systemprompt_logging::{AiRequestInfo, McpToolExecution};

// ============================================================================
// AiRequestInfo Tests
// ============================================================================

#[test]
fn test_ai_request_info_creation() {
    let info = AiRequestInfo {
        id: "req-123".to_string().into(),
        provider: "anthropic".to_string(),
        model: "claude-3".to_string(),
        max_tokens: Some(4096),
        input_tokens: Some(500),
        output_tokens: Some(300),
        cost_microdollars: 5,
        latency_ms: Some(1200),
    };

    assert_eq!(info.id, "req-123");
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.model, "claude-3");
    assert_eq!(info.max_tokens, Some(4096));
    assert_eq!(info.cost_microdollars, 5);
}

#[test]
fn test_ai_request_info_minimal() {
    let info = AiRequestInfo {
        id: "req-min".to_string().into(),
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_microdollars: 0,
        latency_ms: None,
    };

    assert!(info.max_tokens.is_none());
    assert!(info.input_tokens.is_none());
    assert!(info.latency_ms.is_none());
}

#[test]
fn test_ai_request_info_clone() {
    let info = AiRequestInfo {
        id: "clone".to_string().into(),
        provider: "anthropic".to_string(),
        model: "claude".to_string(),
        max_tokens: Some(1000),
        input_tokens: Some(100),
        output_tokens: Some(200),
        cost_microdollars: 3,
        latency_ms: Some(500),
    };

    let cloned = info.clone();
    assert_eq!(info.id, cloned.id);
    assert_eq!(info.provider, cloned.provider);
}

#[test]
fn test_ai_request_info_serialize() {
    let info = AiRequestInfo {
        id: "ser".to_string().into(),
        provider: "test".to_string(),
        model: "model".to_string(),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_microdollars: 1,
        latency_ms: None,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("provider"));
    assert!(json.contains("model"));
}

// ============================================================================
// McpToolExecution Tests
// ============================================================================

#[test]
fn test_mcp_tool_execution_creation() {
    let exec = McpToolExecution {
        mcp_execution_id: "exec-123".to_string().into(),
        tool_name: "file_reader".to_string(),
        server_name: "filesystem".to_string(),
        status: "success".to_string(),
        execution_time_ms: Some(250),
        error_message: None,
        input: r#"{"path": "/tmp/test.txt"}"#.to_string(),
        output: Some("File contents here".to_string()),
    };

    assert_eq!(exec.mcp_execution_id, "exec-123");
    assert_eq!(exec.tool_name, "file_reader");
    assert_eq!(exec.server_name, "filesystem");
    assert_eq!(exec.status, "success");
    assert!(exec.output.is_some());
}

#[test]
fn test_mcp_tool_execution_with_error() {
    let exec = McpToolExecution {
        mcp_execution_id: "exec-err".to_string().into(),
        tool_name: "database_query".to_string(),
        server_name: "postgres".to_string(),
        status: "error".to_string(),
        execution_time_ms: Some(100),
        error_message: Some("Connection refused".to_string()),
        input: r#"{"query": "SELECT *"}"#.to_string(),
        output: None,
    };

    assert_eq!(exec.status, "error");
    assert!(exec.error_message.is_some());
    assert!(exec.output.is_none());
}

#[test]
fn test_mcp_tool_execution_clone() {
    let exec = McpToolExecution {
        mcp_execution_id: "clone".to_string().into(),
        tool_name: "tool".to_string(),
        server_name: "server".to_string(),
        status: "success".to_string(),
        execution_time_ms: Some(500),
        error_message: None,
        input: "input".to_string(),
        output: Some("output".to_string()),
    };

    let cloned = exec.clone();
    assert_eq!(exec.mcp_execution_id, cloned.mcp_execution_id);
    assert_eq!(exec.tool_name, cloned.tool_name);
}

#[test]
fn test_mcp_tool_execution_serialize() {
    let exec = McpToolExecution {
        mcp_execution_id: "ser".to_string().into(),
        tool_name: "tool".to_string(),
        server_name: "server".to_string(),
        status: "pending".to_string(),
        execution_time_ms: None,
        error_message: None,
        input: "{}".to_string(),
        output: None,
    };

    let json = serde_json::to_string(&exec).unwrap();
    assert!(json.contains("tool_name"));
    assert!(json.contains("server_name"));
}
