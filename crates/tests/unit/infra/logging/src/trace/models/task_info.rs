//! Unit tests for TaskInfo and ExecutionStep structs

use chrono::Utc;
use systemprompt_logging::{ExecutionStep, TaskInfo};

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
    assert!(task.execution_time_ms.is_some());
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
    assert!(task.error_message.is_some());
    assert!(task.error_message.as_ref().unwrap().contains("timeout"));
}

#[test]
fn test_task_info_clone() {
    let task = TaskInfo {
        task_id: "clone".to_string().into(),
        context_id: "ctx".to_string().into(),
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

#[test]
fn test_task_info_roundtrip() {
    let task = TaskInfo {
        task_id: "roundtrip".to_string().into(),
        context_id: "ctx".to_string().into(),
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
    assert!(step.error_message.is_some());
}

#[test]
fn test_execution_step_clone() {
    let step = ExecutionStep {
        step_id: "clone".to_string().into(),
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
