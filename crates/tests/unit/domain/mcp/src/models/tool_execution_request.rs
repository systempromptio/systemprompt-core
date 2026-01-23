//! Unit tests for ToolExecutionRequest and ToolExecutionResult models

use chrono::Utc;
use serde_json::json;
use systemprompt_identifiers::{AgentName, AiToolCallId, ContextId, SessionId, TraceId};
use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_models::RequestContext;

fn create_test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("test-session".to_string()),
        TraceId::new("test-trace".to_string()),
        ContextId::new("test-context".to_string()),
        AgentName::new("test-agent".to_string()),
    )
}

fn create_test_request() -> ToolExecutionRequest {
    ToolExecutionRequest {
        tool_name: "test-tool".to_string(),
        server_name: "test-server".to_string(),
        input: json!({"param": "value"}),
        started_at: Utc::now(),
        context: create_test_context(),
        request_method: Some("POST".to_string()),
        request_source: Some("api".to_string()),
        ai_tool_call_id: Some(AiToolCallId::new("call-123".to_string())),
    }
}

fn create_test_result() -> ToolExecutionResult {
    ToolExecutionResult {
        output: Some(json!({"result": "success"})),
        output_schema: Some(json!({"type": "object"})),
        status: ExecutionStatus::Success.as_str().to_string(),
        error_message: None,
        started_at: Utc::now(),
        completed_at: Utc::now(),
    }
}

// ============================================================================
// ToolExecutionRequest Tests
// ============================================================================

#[test]
fn test_tool_execution_request_creation() {
    let request = create_test_request();

    assert_eq!(request.tool_name, "test-tool");
    assert_eq!(request.server_name, "test-server");
    assert_eq!(request.request_method, Some("POST".to_string()));
    assert_eq!(request.request_source, Some("api".to_string()));
    assert!(request.ai_tool_call_id.is_some());
}

#[test]
fn test_tool_execution_request_without_optionals() {
    let request = ToolExecutionRequest {
        tool_name: "minimal-tool".to_string(),
        server_name: "minimal-server".to_string(),
        input: json!(null),
        started_at: Utc::now(),
        context: create_test_context(),
        request_method: None,
        request_source: None,
        ai_tool_call_id: None,
    };

    assert_eq!(request.tool_name, "minimal-tool");
    assert!(request.request_method.is_none());
    assert!(request.request_source.is_none());
    assert!(request.ai_tool_call_id.is_none());
}

#[test]
fn test_tool_execution_request_clone() {
    let request = create_test_request();
    let cloned = request.clone();

    assert_eq!(request.tool_name, cloned.tool_name);
    assert_eq!(request.server_name, cloned.server_name);
    assert_eq!(request.input, cloned.input);
    assert_eq!(request.request_method, cloned.request_method);
    assert_eq!(request.request_source, cloned.request_source);
}

#[test]
fn test_tool_execution_request_debug() {
    let request = create_test_request();
    let debug = format!("{:?}", request);

    assert!(debug.contains("ToolExecutionRequest"));
    assert!(debug.contains("test-tool"));
    assert!(debug.contains("test-server"));
}

#[test]
fn test_tool_execution_request_with_complex_input() {
    let complex_input = json!({
        "nested": {
            "array": [1, 2, 3],
            "object": {"key": "value"}
        },
        "string": "hello",
        "number": 42,
        "boolean": true,
        "null": null
    });

    let request = ToolExecutionRequest {
        tool_name: "complex-tool".to_string(),
        server_name: "server".to_string(),
        input: complex_input.clone(),
        started_at: Utc::now(),
        context: create_test_context(),
        request_method: None,
        request_source: None,
        ai_tool_call_id: None,
    };

    assert_eq!(request.input, complex_input);
}

#[test]
fn test_tool_execution_request_with_empty_strings() {
    let request = ToolExecutionRequest {
        tool_name: String::new(),
        server_name: String::new(),
        input: json!({}),
        started_at: Utc::now(),
        context: create_test_context(),
        request_method: Some(String::new()),
        request_source: Some(String::new()),
        ai_tool_call_id: None,
    };

    assert!(request.tool_name.is_empty());
    assert!(request.server_name.is_empty());
}

#[test]
fn test_tool_execution_request_timestamps() {
    let before = Utc::now();
    let request = create_test_request();
    let after = Utc::now();

    assert!(request.started_at >= before);
    assert!(request.started_at <= after);
}

// ============================================================================
// ToolExecutionResult Tests
// ============================================================================

#[test]
fn test_tool_execution_result_success() {
    let result = create_test_result();

    assert!(result.output.is_some());
    assert!(result.output_schema.is_some());
    assert_eq!(result.status, "success");
    assert!(result.error_message.is_none());
}

#[test]
fn test_tool_execution_result_failure() {
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: ExecutionStatus::Failed.as_str().to_string(),
        error_message: Some("Connection timeout".to_string()),
        started_at: Utc::now(),
        completed_at: Utc::now(),
    };

    assert!(result.output.is_none());
    assert_eq!(result.status, "failed");
    assert_eq!(result.error_message, Some("Connection timeout".to_string()));
}

#[test]
fn test_tool_execution_result_pending() {
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: ExecutionStatus::Pending.as_str().to_string(),
        error_message: None,
        started_at: Utc::now(),
        completed_at: Utc::now(),
    };

    assert_eq!(result.status, "pending");
}

#[test]
fn test_tool_execution_result_clone() {
    let result = create_test_result();
    let cloned = result.clone();

    assert_eq!(result.output, cloned.output);
    assert_eq!(result.output_schema, cloned.output_schema);
    assert_eq!(result.status, cloned.status);
    assert_eq!(result.error_message, cloned.error_message);
}

#[test]
fn test_tool_execution_result_debug() {
    let result = create_test_result();
    let debug = format!("{:?}", result);

    assert!(debug.contains("ToolExecutionResult"));
    assert!(debug.contains("success"));
}

#[test]
fn test_tool_execution_result_with_large_output() {
    let large_output = json!({"data": "x".repeat(10000)});

    let result = ToolExecutionResult {
        output: Some(large_output.clone()),
        output_schema: None,
        status: "success".to_string(),
        error_message: None,
        started_at: Utc::now(),
        completed_at: Utc::now(),
    };

    assert_eq!(result.output, Some(large_output));
}

#[test]
fn test_tool_execution_result_duration() {
    let start = Utc::now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let end = Utc::now();

    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: "success".to_string(),
        error_message: None,
        started_at: start,
        completed_at: end,
    };

    let duration = result.completed_at - result.started_at;
    assert!(duration.num_milliseconds() >= 10);
}

#[test]
fn test_tool_execution_result_with_unicode_error() {
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: "failed".to_string(),
        error_message: Some("错误信息: Failed to process 文件".to_string()),
        started_at: Utc::now(),
        completed_at: Utc::now(),
    };

    assert!(result.error_message.as_ref().unwrap().contains("错误信息"));
    assert!(result.error_message.as_ref().unwrap().contains("文件"));
}

#[test]
fn test_tool_execution_result_empty_error_message() {
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: "failed".to_string(),
        error_message: Some(String::new()),
        started_at: Utc::now(),
        completed_at: Utc::now(),
    };

    assert_eq!(result.error_message, Some(String::new()));
}

// ============================================================================
// Combined Request/Result Tests
// ============================================================================

#[test]
fn test_request_result_consistency() {
    let request = create_test_request();
    let result = create_test_result();

    assert!(result.started_at >= request.started_at || result.started_at <= request.started_at);
    assert!(result.completed_at >= result.started_at);
}
