use systemprompt_agent::models::a2a::protocol::{
    StreamResponse, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part, TaskState, TaskStatus};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

fn context_id() -> ContextId {
    ContextId::try_new(TEST_CONTEXT_ID_A).expect("valid ContextId")
}

fn create_test_artifact(id: &str) -> Artifact {
    Artifact {
        id: ArtifactId::new(id),
        title: Some("test-artifact".to_string()),
        description: Some("Test artifact description".to_string()),
        parts: vec![],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_string(), context_id(), TaskId::new("task-1")),
    }
}

fn status(state: TaskState) -> TaskStatus {
    TaskStatus {
        state,
        ..TaskStatus::default()
    }
}

#[test]
fn test_task_status_update_event_new() {
    let event = TaskStatusUpdateEvent::new(
        TaskId::new("task-123"),
        context_id(),
        status(TaskState::Working),
    );

    assert_eq!(event.task_id, "task-123");
    assert_eq!(event.context_id.as_str(), TEST_CONTEXT_ID_A);
    assert!(event.metadata.is_none());
}

#[test]
fn test_task_status_update_event_serialize_has_no_kind_or_final() {
    let event = TaskStatusUpdateEvent::new(
        TaskId::new("task-1"),
        context_id(),
        status(TaskState::Failed),
    );
    let value = serde_json::to_value(&event).unwrap();

    assert_eq!(value["taskId"], "task-1");
    assert_eq!(value["contextId"], TEST_CONTEXT_ID_A);
    assert_eq!(value["status"]["state"], "TASK_STATE_FAILED");
    assert!(value.get("kind").is_none());
    assert!(value.get("final").is_none());
}

#[test]
fn test_status_update_stream_response_is_tagged() {
    let event = TaskStatusUpdateEvent::new(
        TaskId::new("t1"),
        context_id(),
        status(TaskState::Completed),
    );
    let value = serde_json::to_value(StreamResponse::StatusUpdate(event)).unwrap();

    assert_eq!(
        value["statusUpdate"]["status"]["state"],
        "TASK_STATE_COMPLETED"
    );
}

#[test]
fn test_task_artifact_update_event_new_defaults_to_single_chunk() {
    let event = TaskArtifactUpdateEvent::new(
        TaskId::new("task-1"),
        context_id(),
        create_test_artifact("art-1"),
    );

    assert_eq!(event.task_id, "task-1");
    assert!(!event.append);
    assert!(event.last_chunk);
}

#[test]
fn test_task_artifact_update_event_chunking() {
    let mut artifact = create_test_artifact("art-2");
    artifact.parts = vec![Part::text("Some content")];

    let event = TaskArtifactUpdateEvent::new(TaskId::new("task-2"), context_id(), artifact)
        .chunk(true, false);

    assert!(event.append);
    assert!(!event.last_chunk);
    assert_eq!(event.artifact.parts.len(), 1);
}

#[test]
fn test_task_artifact_update_event_serialize() {
    let event = TaskArtifactUpdateEvent::new(
        TaskId::new("t1"),
        context_id(),
        create_test_artifact("art-3"),
    );
    let value = serde_json::to_value(StreamResponse::ArtifactUpdate(event)).unwrap();
    let inner = &value["artifactUpdate"];

    assert_eq!(inner["taskId"], "t1");
    assert_eq!(inner["artifact"]["artifactId"], "art-3");
    assert_eq!(inner["lastChunk"], true);
    assert!(inner.get("kind").is_none());
}
