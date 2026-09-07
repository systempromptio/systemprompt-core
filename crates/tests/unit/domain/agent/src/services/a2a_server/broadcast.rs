// Tests for the lifecycle webhook broadcasts. With no broadcast endpoint
// listening, the connection fails: the task-event broadcasts log and swallow
// the error (returning unit), while `broadcast_artifact_created` surfaces it as
// an `AgentError`. `ensure_test_bootstrap` initialises the global `Config` so
// the api_internal_url lookup inside each function succeeds.

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use systemprompt_agent::services::a2a_server::streaming::broadcast::{
    BroadcastTaskCreatedParams, broadcast_artifact_created, broadcast_task_completed,
    broadcast_task_created,
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

async fn coverage_task_webhook(status: u16) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/webhook/broadcast"))
        .respond_with(ResponseTemplate::new(status))
        .expect(2)
        .mount(&server)
        .await;
    systemprompt_test_fixtures::init_isolated_bootstrap(&server.uri(), "mcp_servers: {}\n");
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let user = UserId::new("webhook-owner");
    let message = user_message(&ctx);
    broadcast_task_created(BroadcastTaskCreatedParams {
        task_id: &task_id,
        context_id: &ctx,
        user_id: user.as_str(),
        user_message: &message,
        agent_name: "webhook-agent",
        token: "webhook-token",
    })
    .await;
    broadcast_task_completed(&completed_task(&task_id, &ctx), &user, "webhook-token").await;
    let requests = server.received_requests().await.unwrap();
    for (request, event) in requests.iter().zip(["task_created", "task_completed"]) {
        assert_eq!(request.headers["authorization"], "Bearer webhook-token");
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["event_type"], event);
        assert_eq!(body["entity_id"], task_id.as_str());
        assert_eq!(body["context_id"], ctx.as_str());
        assert_eq!(body["user_id"], user.as_str());
        assert!(body["task_data"].is_object());
    }
}
#[tokio::test]
async fn coverage_task_lifecycle_webhooks_forward_trace_identity_and_bearer() {
    coverage_task_webhook(200).await;
}
#[tokio::test]
async fn coverage_rejected_task_webhooks_do_not_abort_task_lifecycle() {
    coverage_task_webhook(503).await;
}
#[tokio::test]
async fn coverage_missing_config_does_not_panic_for_best_effort_task_broadcasts() {
    assert!(systemprompt_models::Config::get().is_err());
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let message = user_message(&ctx);
    broadcast_task_created(BroadcastTaskCreatedParams {
        task_id: &task_id,
        context_id: &ctx,
        user_id: "user",
        user_message: &message,
        agent_name: "agent",
        token: "token",
    })
    .await;
    broadcast_task_completed(
        &completed_task(&task_id, &ctx),
        &UserId::new("user"),
        "token",
    )
    .await;
}
