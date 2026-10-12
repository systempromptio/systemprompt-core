use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::services::a2a_server::errors::JsonRpcErrorBuilder;
use systemprompt_agent::services::a2a_server::streaming::{
    jsonrpc_error_event, stream_response_frame,
};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, SendMessageResult, StreamResponse,
    Task, TaskArtifactUpdateEvent, TaskState, TaskStatus, TaskStatusUpdateEvent,
};

use super::schema::{assert_invalid, assert_valid};

fn ids() -> (TaskId, ContextId) {
    (
        TaskId::new("task-1"),
        ContextId::try_new("00000000-0000-4000-8000-000000000001").unwrap(),
    )
}

fn status(state: TaskState) -> TaskStatus {
    TaskStatus {
        state,
        message: None,
        timestamp: Some(chrono::Utc::now()),
    }
}

fn artifact() -> Artifact {
    let (task_id, context_id) = ids();
    Artifact {
        id: ArtifactId::new("art-1"),
        title: Some("result".to_string()),
        description: None,
        parts: vec![Part::text("hello")],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_string(), context_id, task_id),
    }
}

fn frame(response: StreamResponse) -> serde_json::Value {
    serde_json::to_value(stream_response_frame(response, &RequestId::Number(42))).unwrap()
}

#[test]
fn status_update_frame_is_a_jsonrpc_result_carrying_the_request_id() {
    let (task_id, context_id) = ids();
    let json = frame(StreamResponse::StatusUpdate(TaskStatusUpdateEvent::new(
        task_id,
        context_id,
        status(TaskState::Working),
    )));
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 42);
    assert!(json.get("error").is_none());
    let update = &json["result"]["statusUpdate"];
    assert_eq!(update["taskId"], "task-1");
    assert_eq!(update["status"]["state"], "TASK_STATE_WORKING");
    assert!(
        update.get("final").is_none(),
        "1.0 has no in-band final flag"
    );
    assert!(update.get("kind").is_none());
    assert_valid("StreamResponse", &json["result"]);
}

#[test]
fn terminal_status_frames_validate() {
    let (task_id, context_id) = ids();
    for state in [TaskState::Completed, TaskState::Failed, TaskState::Canceled] {
        let json = frame(StreamResponse::StatusUpdate(TaskStatusUpdateEvent::new(
            task_id.clone(),
            context_id.clone(),
            status(state),
        )));
        assert_valid("StreamResponse", &json["result"]);
    }
}

#[test]
fn artifact_update_frame_is_a_last_chunk() {
    let (task_id, context_id) = ids();
    let json = frame(StreamResponse::ArtifactUpdate(
        TaskArtifactUpdateEvent::new(task_id, context_id, artifact()),
    ));
    let update = &json["result"]["artifactUpdate"];
    assert_eq!(update["lastChunk"], true);
    assert_eq!(update["append"], false);
    assert_eq!(update["artifact"]["artifactId"], "art-1");
    assert_valid("StreamResponse", &json["result"]);
    assert_valid("TaskArtifactUpdateEvent", update);
}

#[test]
fn task_frame_validates() {
    let (task_id, context_id) = ids();
    let task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: status(TaskState::Submitted),
        history: Some(vec![Message {
            role: MessageRole::User,
            parts: vec![Part::text("hi")],
            message_id: MessageId::new("m-1"),
            task_id: Some(task_id),
            context_id,
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        }]),
        ..Task::default()
    };
    let json = frame(StreamResponse::Task(task));
    assert_valid("StreamResponse", &json["result"]);
}

#[test]
fn stream_response_carries_exactly_one_member() {
    let (task_id, context_id) = ids();
    let json = frame(StreamResponse::StatusUpdate(TaskStatusUpdateEvent::new(
        task_id,
        context_id,
        status(TaskState::Working),
    )));
    assert_eq!(json["result"].as_object().unwrap().len(), 1);
    assert_invalid(
        "StreamResponse",
        &serde_json::json!({"statusUpdate": json["result"]["statusUpdate"], "kind": "status-update"}),
    );
}

#[test]
fn send_message_result_wraps_the_task() {
    let result = SendMessageResult::Task(Box::default());
    let json = serde_json::to_value(&result).unwrap();
    assert!(json["task"].is_object());
    assert_valid("SendMessageResponse", &json);
}

#[test]
fn error_frame_is_a_jsonrpc_error_with_error_info() {
    let event = jsonrpc_error_event(
        JsonRpcErrorBuilder::internal_error().with_metadata("detail", "Agent not found"),
        &RequestId::String("s-1".into()),
    );
    let rendered = format!("{event:?}");
    assert!(rendered.contains("google.rpc.ErrorInfo"), "{rendered}");
    assert!(rendered.contains("s-1"), "{rendered}");
}
