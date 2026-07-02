// DB-backed tests for the non-streaming message pipeline
// (`MessageProcessor::handle_message_with_runtime`): full run against a
// stubbed provider (task persisted, completed, response text captured),
// task-id reuse from the inbound message, and the context-validation
// failure path.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::processing::message::MessageProcessor;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn user_message(ctx: &ContextId, task_id: Option<TaskId>, text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id,
        context_id: ctx.clone(),
        metadata: Some(serde_json::json!({"clientMessageId": "cm-1"})),
        extensions: None,
        reference_task_ids: None,
    }
}

#[tokio::test]
async fn handle_message_with_runtime_completes_task_end_to_end() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["It is ", "42."]));
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&ctx, None, "what is the answer?");

    let task = processor
        .handle_message_with_runtime(msg, &runtime, "nonstream-agent", &request)
        .await
        .expect("handled");

    assert_eq!(task.context_id, ctx);
    assert_eq!(task.status.state, TaskState::Completed);
    let agent_msg = task.status.message.as_ref().expect("agent message");
    let Part::Text(text) = &agent_msg.parts[0] else {
        panic!("expected text part");
    };
    assert!(text.text.contains("42"));

    let stored = repos
        .tasks
        .get_task(&task.id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Completed);
}

#[tokio::test]
async fn handle_message_with_runtime_reuses_inbound_task_id() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;
    let client_task_id = TaskId::generate();

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["continuing"]));
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&ctx, Some(client_task_id.clone()), "more");

    let task = processor
        .handle_message_with_runtime(msg, &runtime, "nonstream-agent", &request)
        .await
        .expect("handled");

    assert_eq!(task.id, client_task_id);
}

#[tokio::test]
async fn handle_message_with_runtime_rejects_unowned_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let (user, session) = seed_user_and_session(&pool).await;

    let provider = Arc::new(StubAiProvider::new());
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let foreign_ctx = ContextId::generate();
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&foreign_ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&foreign_ctx, None, "hi");

    let err = processor
        .handle_message_with_runtime(msg, &runtime, "nonstream-agent", &request)
        .await
        .expect_err("unowned context must fail");
    assert!(err.to_string().contains("Context validation failed"));
}
