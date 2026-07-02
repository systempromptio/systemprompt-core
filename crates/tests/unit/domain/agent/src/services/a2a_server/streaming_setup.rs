// Drives create_sse_stream through stream setup with a real seeded context:
// context validation passes, the initial task is persisted, the callback
// push-notification config is stored, and agent-runtime loading then fails
// against the empty fixture registry — the task is marked failed and an
// "Agent not found" JSON-RPC error event is emitted on the stream.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::PushNotificationConfig;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::streaming::{
    CreateSseStreamParams, create_sse_stream,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn message(ctx: &ContextId, task_id: Option<TaskId>) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "stream this".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id,
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

async fn collect_events(
    stream: impl futures::Stream<Item = axum::response::sse::Event> + Send,
) -> Vec<String> {
    let mut events = Vec::new();
    let mut stream = std::pin::pin!(stream);
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
        events.push(format!("{event:?}"));
        if events.len() > 32 {
            break;
        }
    }
    events
}

#[tokio::test]
async fn setup_with_valid_context_persists_task_and_reports_missing_agent() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing_task) = seed_context_and_task(&repos_handle, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx, Some(task_id.clone())),
        agent_name: "test_agent".to_owned(),
        state,
        request_id: RequestId::Number(2),
        context,
        callback_config: Some(PushNotificationConfig {
            endpoint: String::new(),
            headers: None,
            url: "https://example.invalid/callback".to_owned(),
            token: Some("cb".to_owned()),
            authentication: None,
        }),
    })
    .await
    .map_err(|_| ())
    .expect("permit available");

    let events = collect_events(stream).await;
    assert!(
        events.iter().any(|e| e.contains("Agent not found")),
        "expected agent-load failure event, got {events:?}"
    );

    let stored = repos_handle
        .tasks
        .get_task(&task_id)
        .await
        .expect("get task");
    assert!(stored.is_some(), "initial task must have been persisted");
}

#[tokio::test]
async fn setup_without_task_id_mints_one_and_validates_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _task) = seed_context_and_task(&repos_handle, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let context = request_context(&ctx, &session, &user, "other_agent");

    let stream = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx, None),
        agent_name: "test_agent".to_owned(),
        state,
        request_id: RequestId::String("stream-2".to_owned()),
        context,
        callback_config: None,
    })
    .await
    .map_err(|_| ())
    .expect("permit available");

    let events = collect_events(stream).await;
    assert!(
        events.iter().any(|e| e.contains("Agent not found")),
        "expected agent-load failure event, got {events:?}"
    );
}
