//! Tests for TaskInfo, ExecutionStep, AiRequestInfo, McpToolExecution

use chrono::Utc;
use systemprompt_logging::{AiRequestInfo, ExecutionStep, McpToolExecution, TaskInfo};

// ============================================================================
// TaskInfo Tests
// ============================================================================

#[test]
fn test_task_info_creation() {
    let task = TaskInfo {
        task_id: "task-123".to_string().into(),
        context_id: "ctx-456".to_string().into(),
        agent_name: Some("test-agent".to_string()),
        status: "completed".to_string(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        execution_time_ms: Some(5000),
        error_message: None,
    };

    assert_eq!(task.task_id, "task-123");
    assert_eq!(task.context_id, "ctx-456");
    assert_eq!(task.agent_name, Some("test-agent".to_string()));
    assert_eq!(task.status, "completed");
    task.execution_time_ms
        .expect("task.execution_time_ms should be present");
}

#[test]
fn test_task_info_minimal() {
    let task = TaskInfo {
        task_id: "task-min".to_string().into(),
        context_id: "ctx-min".to_string().into(),
        agent_name: None,
        status: "pending".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        execution_time_ms: None,
        error_message: None,
    };

    assert!(task.agent_name.is_none());
    assert!(task.started_at.is_none());
    assert!(task.completed_at.is_none());
}

#[test]
fn test_task_info_with_error() {
    let task = TaskInfo {
        task_id: "task-err".to_string().into(),
        context_id: "ctx-err".to_string().into(),
        agent_name: Some("error-agent".to_string()),
        status: "failed".to_string(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        execution_time_ms: Some(100),
        error_message: Some("Task failed due to timeout".to_string()),
    };

    assert_eq!(task.status, "failed");
    task.error_message
        .as_ref()
        .expect("task.error_message should be present");
    assert!(task.error_message.as_ref().unwrap().contains("timeout"));
}

#[test]
fn test_task_info_serialize() {
    let task = TaskInfo {
        task_id: "ser".to_string().into(),
        context_id: "ctx".to_string().into(),
        agent_name: None,
        status: "pending".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        execution_time_ms: None,
        error_message: None,
    };

    let json = serde_json::to_string(&task).unwrap();
    assert!(json.contains("task_id"));
    assert!(json.contains("pending"));
}

// ============================================================================
// ExecutionStep Tests
// ============================================================================

#[test]
fn test_execution_step_creation() {
    let step = ExecutionStep {
        step_id: "step-123".to_string().into(),
        step_type: Some("analysis".to_string()),
        title: Some("Analyze input".to_string()),
        status: "completed".to_string(),
        duration_ms: Some(1500),
        error_message: None,
    };

    assert_eq!(step.step_id, "step-123");
    assert_eq!(step.step_type, Some("analysis".to_string()));
    assert_eq!(step.title, Some("Analyze input".to_string()));
    assert_eq!(step.status, "completed");
    assert_eq!(step.duration_ms, Some(1500));
}

#[test]
fn test_execution_step_minimal() {
    let step = ExecutionStep {
        step_id: "step-min".to_string().into(),
        step_type: None,
        title: None,
        status: "pending".to_string(),
        duration_ms: None,
        error_message: None,
    };

    assert!(step.step_type.is_none());
    assert!(step.title.is_none());
    assert!(step.duration_ms.is_none());
}

#[test]
fn test_execution_step_with_error() {
    let step = ExecutionStep {
        step_id: "step-err".to_string().into(),
        step_type: Some("processing".to_string()),
        title: Some("Process data".to_string()),
        status: "failed".to_string(),
        duration_ms: Some(500),
        error_message: Some("Processing error occurred".to_string()),
    };

    assert_eq!(step.status, "failed");
    step.error_message
        .expect("step.error_message should be present");
}

#[test]
fn test_execution_step_serialize() {
    let step = ExecutionStep {
        step_id: "ser".to_string().into(),
        step_type: None,
        title: None,
        status: "pending".to_string(),
        duration_ms: None,
        error_message: None,
    };

    let json = serde_json::to_string(&step).unwrap();
    assert!(json.contains("step_id"));
    assert!(json.contains("pending"));
}

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
    exec.output.expect("exec.output should be present");
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
    exec.error_message
        .expect("exec.error_message should be present");
    assert!(exec.output.is_none());
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
