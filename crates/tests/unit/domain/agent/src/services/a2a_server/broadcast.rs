// Tests for the lifecycle webhook broadcasts. With no broadcast endpoint
// listening, the connection fails: the task-event broadcasts log and swallow
// the error (returning unit), while `broadcast_artifact_created` surfaces it as
// an `AgentError`. `ensure_test_bootstrap` initialises the global `Config` so
// the api_internal_url lookup inside each function succeeds.

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use systemprompt_agent::services::a2a_server::streaming::broadcast::{
    broadcast_artifact_created, broadcast_task_completed, broadcast_task_created,
    BroadcastTaskCreatedParams,
};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId, UserId};

fn user_message(ctx: &ContextId) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "hello".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn completed_task(task_id: &TaskId, ctx: &ContextId) -> Task {
    Task {
        id: task_id.clone(),
        context_id: ctx.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: None,
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

#[tokio::test]
async fn broadcast_task_created_swallows_transport_failure() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let msg = user_message(&ctx);

    // No webhook is listening; the function must return without panicking.
    broadcast_task_created(BroadcastTaskCreatedParams {
        task_id: &task_id,
        context_id: &ctx,
        user_id: "u-bcast",
        user_message: &msg,
        agent_name: "bcast-agent",
        token: "tok",
    })
    .await;
}

#[tokio::test]
async fn broadcast_task_completed_swallows_transport_failure() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let task = completed_task(&task_id, &ctx);

    broadcast_task_completed(&task, &UserId::new("u-bcast"), "tok").await;
}

#[tokio::test]
async fn broadcast_artifact_created_surfaces_transport_error() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: Some("Doc".to_owned()),
        description: None,
        parts: Vec::new(),
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new("document".to_owned(), ctx.clone(), task_id.clone()),
    };

    let result =
        broadcast_artifact_created(&artifact, &task_id, &ctx, &UserId::new("u-bcast"), "tok").await;
    assert!(
        result.is_err(),
        "expected webhook transport error to surface as AgentError"
    );
}
