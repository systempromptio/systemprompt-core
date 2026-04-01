//! Tests for TraceEvent, AiRequestSummary, McpExecutionSummary, ExecutionStepSummary

use chrono::Utc;
use systemprompt_logging::{
    AiRequestSummary, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
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

// ============================================================================
// AiRequestSummary Tests
// ============================================================================

#[test]
fn test_ai_request_summary_default() {
    let summary = AiRequestSummary::default();

    assert_eq!(summary.total_cost_microdollars, 0);
    assert_eq!(summary.total_tokens, 0);
    assert_eq!(summary.total_input_tokens, 0);
    assert_eq!(summary.total_output_tokens, 0);
    assert_eq!(summary.request_count, 0);
    assert_eq!(summary.total_latency_ms, 0);
}

#[test]
fn test_ai_request_summary_creation() {
    let summary = AiRequestSummary {
        total_cost_microdollars: 100,
        total_tokens: 5000,
        total_input_tokens: 3000,
        total_output_tokens: 2000,
        request_count: 10,
        total_latency_ms: 15000,
    };

    assert_eq!(summary.total_cost_microdollars, 100);
    assert_eq!(summary.total_tokens, 5000);
    assert_eq!(summary.total_input_tokens, 3000);
    assert_eq!(summary.total_output_tokens, 2000);
    assert_eq!(summary.request_count, 10);
    assert_eq!(summary.total_latency_ms, 15000);
}

#[test]
fn test_ai_request_summary_clone() {
    let summary = AiRequestSummary {
        total_cost_microdollars: 50,
        total_tokens: 1000,
        total_input_tokens: 600,
        total_output_tokens: 400,
        request_count: 5,
        total_latency_ms: 5000,
    };

    let cloned = summary.clone();
    assert_eq!(summary.total_cost_microdollars, cloned.total_cost_microdollars);
    assert_eq!(summary.request_count, cloned.request_count);
}

#[test]
fn test_ai_request_summary_copy() {
    let summary = AiRequestSummary::default();
    let copied: AiRequestSummary = summary;
    assert_eq!(summary.total_cost_microdollars, copied.total_cost_microdollars);
}

#[test]
fn test_ai_request_summary_serialize() {
    let summary = AiRequestSummary {
        total_cost_microdollars: 25,
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
        "total_cost_microdollars": 100,
        "total_tokens": 2000,
        "total_input_tokens": 1200,
        "total_output_tokens": 800,
        "request_count": 5,
        "total_latency_ms": 3000
    }"#;

    let summary: AiRequestSummary = serde_json::from_str(json).unwrap();
    assert_eq!(summary.total_cost_microdollars, 100);
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
