//! Unit tests for trace models

use chrono::Utc;
use serde_json::json;
use systemprompt_logging::{
    AiRequestInfo, AiRequestSummary, ConversationMessage, ExecutionStep, ExecutionStepSummary,
    McpExecutionSummary, McpToolExecution, TaskArtifact, TaskInfo, ToolLogEntry, TraceEvent,
};

// ============================================================================
// TraceEvent Tests
// ============================================================================

#[test]
fn test_trace_event_creation() {
    let event = TraceEvent {
        event_type: "test_event".to_string(),
        timestamp: Utc::now(),
        details: "Test details".to_string(),
        user_id: Some("user-123".to_string()),
        session_id: Some("session-456".to_string()),
        task_id: Some("task-789".to_string()),
        context_id: Some("context-abc".to_string()),
        metadata: Some(r#"{"key": "value"}"#.to_string()),
    };

    assert_eq!(event.event_type, "test_event");
    assert_eq!(event.details, "Test details");
    assert_eq!(event.user_id, Some("user-123".to_string()));
    assert_eq!(event.session_id, Some("session-456".to_string()));
    assert_eq!(event.task_id, Some("task-789".to_string()));
    assert_eq!(event.context_id, Some("context-abc".to_string()));
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
fn test_trace_event_debug() {
    let event = TraceEvent {
        event_type: "debug_test".to_string(),
        timestamp: Utc::now(),
        details: "Debug details".to_string(),
        user_id: None,
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    };

    let debug = format!("{:?}", event);
    assert!(debug.contains("TraceEvent"));
    assert!(debug.contains("debug_test"));
}

#[test]
fn test_trace_event_clone() {
    let event = TraceEvent {
        event_type: "clone_test".to_string(),
        timestamp: Utc::now(),
        details: "Clone details".to_string(),
        user_id: Some("user".to_string()),
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

// ============================================================================
// AiRequestSummary Tests
// ============================================================================

#[test]
fn test_ai_request_summary_default() {
    let summary = AiRequestSummary::default();

    assert_eq!(summary.total_cost_cents, 0);
    assert_eq!(summary.total_tokens, 0);
    assert_eq!(summary.total_input_tokens, 0);
    assert_eq!(summary.total_output_tokens, 0);
    assert_eq!(summary.request_count, 0);
    assert_eq!(summary.total_latency_ms, 0);
}

#[test]
fn test_ai_request_summary_creation() {
    let summary = AiRequestSummary {
        total_cost_cents: 100,
        total_tokens: 5000,
        total_input_tokens: 3000,
        total_output_tokens: 2000,
        request_count: 10,
        total_latency_ms: 15000,
    };

    assert_eq!(summary.total_cost_cents, 100);
    assert_eq!(summary.total_tokens, 5000);
    assert_eq!(summary.total_input_tokens, 3000);
    assert_eq!(summary.total_output_tokens, 2000);
    assert_eq!(summary.request_count, 10);
    assert_eq!(summary.total_latency_ms, 15000);
}

#[test]
fn test_ai_request_summary_debug() {
    let summary = AiRequestSummary::default();
    let debug = format!("{:?}", summary);
    assert!(debug.contains("AiRequestSummary"));
}

#[test]
fn test_ai_request_summary_clone() {
    let summary = AiRequestSummary {
        total_cost_cents: 50,
        total_tokens: 1000,
        total_input_tokens: 600,
        total_output_tokens: 400,
        request_count: 5,
        total_latency_ms: 5000,
    };

    let cloned = summary.clone();
    assert_eq!(summary.total_cost_cents, cloned.total_cost_cents);
    assert_eq!(summary.request_count, cloned.request_count);
}

#[test]
fn test_ai_request_summary_copy() {
    let summary = AiRequestSummary::default();
    let copied: AiRequestSummary = summary;
    assert_eq!(summary.total_cost_cents, copied.total_cost_cents);
}

#[test]
fn test_ai_request_summary_serialize() {
    let summary = AiRequestSummary {
        total_cost_cents: 25,
        total_tokens: 500,
        total_input_tokens: 300,
        total_output_tokens: 200,
        request_count: 2,
        total_latency_ms: 1000,
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("25"));
    assert!(json.contains("500"));
}

#[test]
fn test_ai_request_summary_deserialize() {
    let json = r#"{
        "total_cost_cents": 100,
        "total_tokens": 2000,
        "total_input_tokens": 1200,
        "total_output_tokens": 800,
        "request_count": 5,
        "total_latency_ms": 3000
    }"#;

    let summary: AiRequestSummary = serde_json::from_str(json).unwrap();
    assert_eq!(summary.total_cost_cents, 100);
    assert_eq!(summary.total_tokens, 2000);
}

// ============================================================================
// McpExecutionSummary Tests
// ============================================================================

#[test]
fn test_mcp_execution_summary_default() {
    let summary = McpExecutionSummary::default();

    assert_eq!(summary.execution_count, 0);
    assert_eq!(summary.total_execution_time_ms, 0);
}

#[test]
fn test_mcp_execution_summary_creation() {
    let summary = McpExecutionSummary {
        execution_count: 15,
        total_execution_time_ms: 30000,
    };

    assert_eq!(summary.execution_count, 15);
    assert_eq!(summary.total_execution_time_ms, 30000);
}

#[test]
fn test_mcp_execution_summary_debug() {
    let summary = McpExecutionSummary::default();
    let debug = format!("{:?}", summary);
    assert!(debug.contains("McpExecutionSummary"));
}

#[test]
fn test_mcp_execution_summary_clone() {
    let summary = McpExecutionSummary {
        execution_count: 10,
        total_execution_time_ms: 5000,
    };

    let cloned = summary.clone();
    assert_eq!(summary.execution_count, cloned.execution_count);
}

#[test]
fn test_mcp_execution_summary_copy() {
    let summary = McpExecutionSummary::default();
    let copied: McpExecutionSummary = summary;
    assert_eq!(summary.execution_count, copied.execution_count);
}

#[test]
fn test_mcp_execution_summary_serialize() {
    let summary = McpExecutionSummary {
        execution_count: 8,
        total_execution_time_ms: 4000,
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("8"));
    assert!(json.contains("4000"));
}

#[test]
fn test_mcp_execution_summary_deserialize() {
    let json = r#"{
        "execution_count": 20,
        "total_execution_time_ms": 10000
    }"#;

    let summary: McpExecutionSummary = serde_json::from_str(json).unwrap();
    assert_eq!(summary.execution_count, 20);
    assert_eq!(summary.total_execution_time_ms, 10000);
}

// ============================================================================
// ExecutionStepSummary Tests
// ============================================================================

#[test]
fn test_execution_step_summary_default() {
    let summary = ExecutionStepSummary::default();

    assert_eq!(summary.total, 0);
    assert_eq!(summary.completed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.pending, 0);
}

#[test]
fn test_execution_step_summary_creation() {
    let summary = ExecutionStepSummary {
        total: 100,
        completed: 80,
        failed: 5,
        pending: 15,
    };

    assert_eq!(summary.total, 100);
    assert_eq!(summary.completed, 80);
    assert_eq!(summary.failed, 5);
    assert_eq!(summary.pending, 15);
}

#[test]
fn test_execution_step_summary_debug() {
    let summary = ExecutionStepSummary::default();
    let debug = format!("{:?}", summary);
    assert!(debug.contains("ExecutionStepSummary"));
}

#[test]
fn test_execution_step_summary_clone() {
    let summary = ExecutionStepSummary {
        total: 50,
        completed: 40,
        failed: 3,
        pending: 7,
    };

    let cloned = summary.clone();
    assert_eq!(summary.total, cloned.total);
    assert_eq!(summary.completed, cloned.completed);
}

#[test]
fn test_execution_step_summary_copy() {
    let summary = ExecutionStepSummary::default();
    let copied: ExecutionStepSummary = summary;
    assert_eq!(summary.total, copied.total);
}

#[test]
fn test_execution_step_summary_serialize() {
    let summary = ExecutionStepSummary {
        total: 30,
        completed: 25,
        failed: 2,
        pending: 3,
    };

    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("step_count"));
    assert!(json.contains("completed_count"));
    assert!(json.contains("failed_count"));
    assert!(json.contains("pending_count"));
}

#[test]
fn test_execution_step_summary_deserialize() {
    let json = r#"{
        "step_count": 40,
        "completed_count": 35,
        "failed_count": 1,
        "pending_count": 4
    }"#;

    let summary: ExecutionStepSummary = serde_json::from_str(json).unwrap();
    assert_eq!(summary.total, 40);
    assert_eq!(summary.completed, 35);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.pending, 4);
}

// ============================================================================
// TaskInfo Tests
// ============================================================================

#[test]
fn test_task_info_creation() {
    let task = TaskInfo {
        task_id: "task-123".to_string(),
        context_id: "ctx-456".to_string(),
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
    assert!(task.execution_time_ms.is_some());
}

#[test]
fn test_task_info_minimal() {
    let task = TaskInfo {
        task_id: "task-min".to_string(),
        context_id: "ctx-min".to_string(),
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
        task_id: "task-err".to_string(),
        context_id: "ctx-err".to_string(),
        agent_name: Some("error-agent".to_string()),
        status: "failed".to_string(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        execution_time_ms: Some(100),
        error_message: Some("Task failed due to timeout".to_string()),
    };

    assert_eq!(task.status, "failed");
    assert!(task.error_message.is_some());
    assert!(task.error_message.as_ref().unwrap().contains("timeout"));
}

#[test]
fn test_task_info_debug() {
    let task = TaskInfo {
        task_id: "debug".to_string(),
        context_id: "ctx".to_string(),
        agent_name: None,
        status: "running".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        execution_time_ms: None,
        error_message: None,
    };

    let debug = format!("{:?}", task);
    assert!(debug.contains("TaskInfo"));
}

#[test]
fn test_task_info_clone() {
    let task = TaskInfo {
        task_id: "clone".to_string(),
        context_id: "ctx".to_string(),
        agent_name: Some("agent".to_string()),
        status: "running".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        execution_time_ms: None,
        error_message: None,
    };

    let cloned = task.clone();
    assert_eq!(task.task_id, cloned.task_id);
    assert_eq!(task.agent_name, cloned.agent_name);
}

#[test]
fn test_task_info_serialize() {
    let task = TaskInfo {
        task_id: "ser".to_string(),
        context_id: "ctx".to_string(),
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
        step_id: "step-123".to_string(),
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
        step_id: "step-min".to_string(),
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
        step_id: "step-err".to_string(),
        step_type: Some("processing".to_string()),
        title: Some("Process data".to_string()),
        status: "failed".to_string(),
        duration_ms: Some(500),
        error_message: Some("Processing error occurred".to_string()),
    };

    assert_eq!(step.status, "failed");
    assert!(step.error_message.is_some());
}

#[test]
fn test_execution_step_debug() {
    let step = ExecutionStep {
        step_id: "debug".to_string(),
        step_type: None,
        title: None,
        status: "running".to_string(),
        duration_ms: None,
        error_message: None,
    };

    let debug = format!("{:?}", step);
    assert!(debug.contains("ExecutionStep"));
}

#[test]
fn test_execution_step_clone() {
    let step = ExecutionStep {
        step_id: "clone".to_string(),
        step_type: Some("test".to_string()),
        title: Some("Test step".to_string()),
        status: "completed".to_string(),
        duration_ms: Some(100),
        error_message: None,
    };

    let cloned = step.clone();
    assert_eq!(step.step_id, cloned.step_id);
    assert_eq!(step.step_type, cloned.step_type);
}

#[test]
fn test_execution_step_serialize() {
    let step = ExecutionStep {
        step_id: "ser".to_string(),
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
        id: "req-123".to_string(),
        provider: "anthropic".to_string(),
        model: "claude-3".to_string(),
        max_tokens: Some(4096),
        input_tokens: Some(500),
        output_tokens: Some(300),
        cost_cents: 5,
        latency_ms: Some(1200),
    };

    assert_eq!(info.id, "req-123");
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.model, "claude-3");
    assert_eq!(info.max_tokens, Some(4096));
    assert_eq!(info.cost_cents, 5);
}

#[test]
fn test_ai_request_info_minimal() {
    let info = AiRequestInfo {
        id: "req-min".to_string(),
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_cents: 0,
        latency_ms: None,
    };

    assert!(info.max_tokens.is_none());
    assert!(info.input_tokens.is_none());
    assert!(info.latency_ms.is_none());
}

#[test]
fn test_ai_request_info_debug() {
    let info = AiRequestInfo {
        id: "debug".to_string(),
        provider: "test".to_string(),
        model: "test-model".to_string(),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_cents: 0,
        latency_ms: None,
    };

    let debug = format!("{:?}", info);
    assert!(debug.contains("AiRequestInfo"));
}

#[test]
fn test_ai_request_info_clone() {
    let info = AiRequestInfo {
        id: "clone".to_string(),
        provider: "anthropic".to_string(),
        model: "claude".to_string(),
        max_tokens: Some(1000),
        input_tokens: Some(100),
        output_tokens: Some(200),
        cost_cents: 3,
        latency_ms: Some(500),
    };

    let cloned = info.clone();
    assert_eq!(info.id, cloned.id);
    assert_eq!(info.provider, cloned.provider);
}

#[test]
fn test_ai_request_info_serialize() {
    let info = AiRequestInfo {
        id: "ser".to_string(),
        provider: "test".to_string(),
        model: "model".to_string(),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_cents: 1,
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
        mcp_execution_id: "exec-123".to_string(),
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
        mcp_execution_id: "exec-err".to_string(),
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
fn test_mcp_tool_execution_debug() {
    let exec = McpToolExecution {
        mcp_execution_id: "debug".to_string(),
        tool_name: "test".to_string(),
        server_name: "test-server".to_string(),
        status: "pending".to_string(),
        execution_time_ms: None,
        error_message: None,
        input: "{}".to_string(),
        output: None,
    };

    let debug = format!("{:?}", exec);
    assert!(debug.contains("McpToolExecution"));
}

#[test]
fn test_mcp_tool_execution_clone() {
    let exec = McpToolExecution {
        mcp_execution_id: "clone".to_string(),
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
        mcp_execution_id: "ser".to_string(),
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
fn test_conversation_message_debug() {
    let msg = ConversationMessage {
        role: "user".to_string(),
        content: "Debug test".to_string(),
        sequence_number: 1,
    };

    let debug = format!("{:?}", msg);
    assert!(debug.contains("ConversationMessage"));
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

#[test]
fn test_conversation_message_deserialize() {
    let json = r#"{
        "role": "assistant",
        "content": "Deserialized content",
        "sequence_number": 3
    }"#;

    let msg: ConversationMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content, "Deserialized content");
    assert_eq!(msg.sequence_number, 3);
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
fn test_tool_log_entry_debug() {
    let entry = ToolLogEntry {
        timestamp: Utc::now(),
        level: "debug".to_string(),
        module: "test".to_string(),
        message: "Debug message".to_string(),
    };

    let debug = format!("{:?}", entry);
    assert!(debug.contains("ToolLogEntry"));
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
        artifact_id: "art-123".to_string(),
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
    assert!(artifact.text_content.is_some());
}

#[test]
fn test_task_artifact_with_data_content() {
    let artifact = TaskArtifact {
        artifact_id: "art-data".to_string(),
        artifact_type: "json".to_string(),
        name: Some("data.json".to_string()),
        source: None,
        tool_name: None,
        part_kind: Some("data".to_string()),
        text_content: None,
        data_content: Some(json!({"key": "value", "count": 42})),
    };

    assert!(artifact.data_content.is_some());
    let data = artifact.data_content.as_ref().unwrap();
    assert_eq!(data["key"], "value");
    assert_eq!(data["count"], 42);
}

#[test]
fn test_task_artifact_minimal() {
    let artifact = TaskArtifact {
        artifact_id: "art-min".to_string(),
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
fn test_task_artifact_debug() {
    let artifact = TaskArtifact {
        artifact_id: "debug".to_string(),
        artifact_type: "test".to_string(),
        name: None,
        source: None,
        tool_name: None,
        part_kind: None,
        text_content: None,
        data_content: None,
    };

    let debug = format!("{:?}", artifact);
    assert!(debug.contains("TaskArtifact"));
}

#[test]
fn test_task_artifact_clone() {
    let artifact = TaskArtifact {
        artifact_id: "clone".to_string(),
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
        artifact_id: "ser".to_string(),
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

#[test]
fn test_task_artifact_deserialize() {
    let json = r#"{
        "artifact_id": "deser",
        "artifact_type": "output",
        "name": "result.json",
        "source": null,
        "tool_name": null,
        "part_kind": "json",
        "text_content": null,
        "data_content": {"result": true}
    }"#;

    let artifact: TaskArtifact = serde_json::from_str(json).unwrap();
    assert_eq!(artifact.artifact_id, "deser");
    assert_eq!(artifact.name, Some("result.json".to_string()));
    assert!(artifact.data_content.is_some());
}

// ============================================================================
// Roundtrip Serialization Tests
// ============================================================================

#[test]
fn test_trace_event_roundtrip() {
    let event = TraceEvent {
        event_type: "roundtrip".to_string(),
        timestamp: Utc::now(),
        details: "Roundtrip test".to_string(),
        user_id: Some("user".to_string()),
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

#[test]
fn test_ai_request_summary_roundtrip() {
    let summary = AiRequestSummary {
        total_cost_cents: 150,
        total_tokens: 8000,
        total_input_tokens: 5000,
        total_output_tokens: 3000,
        request_count: 20,
        total_latency_ms: 25000,
    };

    let json = serde_json::to_string(&summary).unwrap();
    let deserialized: AiRequestSummary = serde_json::from_str(&json).unwrap();

    assert_eq!(summary.total_cost_cents, deserialized.total_cost_cents);
    assert_eq!(summary.total_tokens, deserialized.total_tokens);
    assert_eq!(summary.request_count, deserialized.request_count);
}

#[test]
fn test_task_info_roundtrip() {
    let task = TaskInfo {
        task_id: "roundtrip".to_string(),
        context_id: "ctx".to_string(),
        agent_name: Some("agent".to_string()),
        status: "completed".to_string(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        execution_time_ms: Some(1000),
        error_message: None,
    };

    let json = serde_json::to_string(&task).unwrap();
    let deserialized: TaskInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(task.task_id, deserialized.task_id);
    assert_eq!(task.status, deserialized.status);
    assert_eq!(task.execution_time_ms, deserialized.execution_time_ms);
}
