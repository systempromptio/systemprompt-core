// Tests for the lifecycle webhook broadcasts through an injected
// `WebhookContext`. The task-event broadcasts log and swallow delivery
// failures (returning unit); `broadcast_artifact_created` surfaces them as a
// `WebhookError` so the caller can record an undelivered broadcast.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use systemprompt_agent::services::a2a_server::streaming::broadcast::{
    BroadcastTaskCreatedParams, broadcast_artifact_created, broadcast_task_completed,
    broadcast_task_created,
};
use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    HttpWebhookBroadcaster, WebhookContext,
};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId, UserId};
use systemprompt_test_mocks::RecordingWebhookBroadcaster;

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

fn artifact(ctx: &ContextId, task_id: &TaskId) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("Doc".to_owned()),
        description: None,
        parts: Vec::new(),
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new("document".to_owned(), ctx.clone(), task_id.clone()),
    }
}

fn http_context(base_url: &str, user: &UserId, token: &str) -> WebhookContext {
    let broadcaster = HttpWebhookBroadcaster::new(base_url).expect("broadcaster");
    WebhookContext::new(Arc::new(broadcaster), user.clone(), token)
}

#[tokio::test]
async fn broadcast_task_created_swallows_transport_failure() {
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let msg = user_message(&ctx);
    let webhooks = http_context("http://127.0.0.1:9", &UserId::new("u-bcast"), "tok");

    // Nothing listens on the discard port; the function must return without
    // panicking.
    broadcast_task_created(BroadcastTaskCreatedParams {
        webhooks: &webhooks,
        task_id: &task_id,
        context_id: &ctx,
        user_message: &msg,
        agent_name: "bcast-agent",
    })
    .await;
}

#[tokio::test]
async fn broadcast_task_completed_swallows_transport_failure() {
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let task = completed_task(&task_id, &ctx);
    let webhooks = http_context("http://127.0.0.1:9", &UserId::new("u-bcast"), "tok");

    broadcast_task_completed(&webhooks, &task).await;
}

#[tokio::test]
async fn broadcast_artifact_created_surfaces_transport_error() {
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let webhooks = http_context("http://127.0.0.1:9", &UserId::new("u-bcast"), "tok");

    let result =
        broadcast_artifact_created(&webhooks, &artifact(&ctx, &task_id), &task_id, &ctx).await;
    assert!(
        result.is_err(),
        "expected webhook transport error to surface as WebhookError"
    );
}

#[tokio::test]
async fn broadcast_artifact_created_surfaces_a_rejected_status() {
    let rec = Arc::new(RecordingWebhookBroadcaster::with_lifecycle_down());
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let webhooks = WebhookContext::new(rec.clone(), UserId::new("u-bcast"), "tok");

    let err = broadcast_artifact_created(&webhooks, &artifact(&ctx, &task_id), &task_id, &ctx)
        .await
        .expect_err("a 503 from the webhook is an error to the caller");
    assert!(err.to_string().contains("503"), "{err}");
    assert_eq!(rec.lifecycle_events().len(), 1);
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
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let user = UserId::new("webhook-owner");
    let message = user_message(&ctx);
    let webhooks = http_context(&server.uri(), &user, "webhook-token");
    broadcast_task_created(BroadcastTaskCreatedParams {
        webhooks: &webhooks,
        task_id: &task_id,
        context_id: &ctx,
        user_message: &message,
        agent_name: "webhook-agent",
    })
    .await;
    broadcast_task_completed(&webhooks, &completed_task(&task_id, &ctx)).await;
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
async fn artifact_created_posts_the_artifact_id_as_the_entity() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/webhook/broadcast"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let ctx = ContextId::generate();
    let task_id = TaskId::generate();
    let user = UserId::new("webhook-owner");
    let artifact = artifact(&ctx, &task_id);
    let webhooks = http_context(&server.uri(), &user, "webhook-token");

    broadcast_artifact_created(&webhooks, &artifact, &task_id, &ctx)
        .await
        .expect("delivered");

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["event_type"], "artifact_created");
    assert_eq!(body["entity_id"], artifact.id.as_str());
    assert_eq!(body["user_id"], user.as_str());
}
