//! Unit tests for A2A protocol event types
//!
//! Tests cover:
//! - TaskStatusUpdateEvent construction and serialization
//! - TaskArtifactUpdateEvent construction and serialization
//! - ServiceStatusParams serialization

use systemprompt_agent::models::a2a::protocol::{TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Part, ServiceStatusParams, TaskState, TaskStatus, TextPart,
};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};

fn create_test_artifact(id: &str) -> Artifact {
    Artifact {
        id: ArtifactId::from(id),
        title: Some("test-artifact".to_string()),
        description: Some("Test artifact description".to_string()),
        parts: vec![],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            "text".to_string(),
            ContextId::from("ctx-1"),
            TaskId::from("task-1"),
        ),
    }
}

fn create_working_status() -> TaskStatus {
    let mut status = TaskStatus::default();
    status.state = TaskState::Working;
    status
}

fn create_completed_status() -> TaskStatus {
    let mut status = TaskStatus::default();
    status.state = TaskState::Completed;
    status
}

fn create_failed_status() -> TaskStatus {
    let mut status = TaskStatus::default();
    status.state = TaskState::Failed;
    status
}

// ============================================================================
// TaskStatusUpdateEvent Tests
// ============================================================================

#[test]
fn test_task_status_update_event_new() {
    let status = create_working_status();
    let event = TaskStatusUpdateEvent::new("task-123", "ctx-456", status, false);

    assert_eq!(event.kind, "status-update");
    assert_eq!(event.task_id, "task-123");
    assert_eq!(event.context_id, "ctx-456");
    assert!(!event.is_final);
}

#[test]
fn test_task_status_update_event_final() {
    let status = create_completed_status();
    let event = TaskStatusUpdateEvent::new("task-abc", "ctx-def", status, true);

    assert!(event.is_final);
    assert!(matches!(event.status.state, TaskState::Completed));
}

#[test]
fn test_task_status_update_event_serialize() {
    let status = create_failed_status();
    let event = TaskStatusUpdateEvent::new("task-1", "ctx-1", status, true);
    let json = serde_json::to_string(&event).unwrap();

    assert!(json.contains("status-update"));
    assert!(json.contains("task-1"));
    assert!(json.contains("ctx-1"));
    assert!(json.contains("TASK_STATE_FAILED"));
}

#[test]
fn test_task_status_update_event_to_jsonrpc_response() {
    let status = TaskStatus::default();
    let event = TaskStatusUpdateEvent::new("t1", "c1", status, false);
    let response = event.to_jsonrpc_response();

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response["result"].is_object());
}

// ============================================================================
// TaskArtifactUpdateEvent Tests
// ============================================================================

#[test]
fn test_task_artifact_update_event_new() {
    let artifact = create_test_artifact("art-1");
    let event = TaskArtifactUpdateEvent::new("task-1", "ctx-1", artifact, false);

    assert_eq!(event.kind, "artifact-update");
    assert_eq!(event.task_id, "task-1");
    assert_eq!(event.context_id, "ctx-1");
    assert!(!event.is_final);
}

#[test]
fn test_task_artifact_update_event_with_parts() {
    let mut artifact = create_test_artifact("art-2");
    artifact.parts = vec![Part::Text(TextPart {
        text: "Some content".to_string(),
    })];

    let event = TaskArtifactUpdateEvent::new("task-2", "ctx-2", artifact, true);

    assert!(event.is_final);
    assert_eq!(event.artifact.parts.len(), 1);
}

#[test]
fn test_task_artifact_update_event_serialize() {
    let artifact = create_test_artifact("art-3");
    let event = TaskArtifactUpdateEvent::new("t1", "c1", artifact, false);
    let json = serde_json::to_string(&event).unwrap();

    assert!(json.contains("artifact-update"));
    assert!(json.contains("t1"));
    assert!(json.contains("c1"));
}

#[test]
fn test_task_artifact_update_event_to_jsonrpc_response() {
    let artifact = create_test_artifact("art-4");
    let event = TaskArtifactUpdateEvent::new("t", "c", artifact, true);
    let response = event.to_jsonrpc_response();

    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response["result"].is_object());
}

// ============================================================================
// ServiceStatusParams Tests
// ============================================================================

#[test]
fn test_service_status_params_serialize() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: true,
        port: Some(8080),
        pid: Some(12345),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("running"));
    assert!(json.contains("8080"));
    assert!(json.contains("12345"));
}

#[test]
fn test_service_status_params_optional_fields() {
    let json = r#"{
        "status": "starting"
    }"#;

    let params: ServiceStatusParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.status, "starting");
    assert!(!params.default);
    assert!(params.port.is_none());
    assert!(params.pid.is_none());
}

#[test]
fn test_service_status_params_debug() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: true,
        port: None,
        pid: None,
    };

    let debug = format!("{:?}", params);
    assert!(debug.contains("ServiceStatusParams"));
    assert!(debug.contains("running"));
}
