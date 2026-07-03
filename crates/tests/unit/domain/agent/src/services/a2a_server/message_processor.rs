// DB-backed tests for `MessageProcessor`: construction, the streaming pipeline
// (`process_message_stream` with a stubbed provider), and persisting a
// completed task (`persist_completed_task`). The streaming path is driven with
// a real seeded context/task and an injected `AgentRuntimeInfo`, so it never
// touches the on-disk agent registry.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::processing::TaskBuilder;
use systemprompt_agent::services::a2a_server::processing::message::{
    MessageProcessor, PersistCompletedTaskOnProcessorParams, ProcessMessageStreamParams,
    StreamEvent,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn user_message(ctx: &ContextId, task_id: &TaskId, text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[tokio::test]
async fn new_constructs_against_pool() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let provider = Arc::new(StubAiProvider::new());
    MessageProcessor::new(&pool, provider).expect("processor constructs");
}

#[tokio::test]
async fn process_message_stream_emits_text_and_complete() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["one ", "two"]));
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let runtime = runtime_info("stream-agent");
    let request = request_context(&ctx, &session, &user, "stream-agent");
    let msg = user_message(&ctx, &task_id, "hi");

    let mut rx = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &msg,
            agent_runtime: &runtime,
            agent_name: "stream-agent",
            context: &request,
            task_id: task_id.clone(),
        })
        .await
        .expect("stream");

    let mut text = String::new();
    let mut completed = false;
    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::Text(t) => text.push_str(&t),
            StreamEvent::Complete { full_text, .. } => {
                assert!(full_text.contains("one"));
                completed = true;
                break;
            },
            StreamEvent::Error(e) => panic!("unexpected error event: {e}"),
            _ => {},
        }
    }
    assert!(completed, "expected a Complete event");
    assert!(text.contains("one") && text.contains("two"));
}

#[tokio::test]
async fn persist_completed_task_updates_existing_row() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new());
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let request = request_context(&ctx, &session, &user, "persist-agent");
    let user_msg = user_message(&ctx, &task_id, "question");

    let task = TaskBuilder::new(ctx.clone())
        .with_task_id(task_id.clone())
        .with_state(TaskState::Completed)
        .with_response_text("the answer".to_owned())
        .with_user_message(user_msg.clone())
        .build();

    let agent_msg = task.status.message.clone().expect("agent message");

    let persisted = processor
        .persist_completed_task(PersistCompletedTaskOnProcessorParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            context: &request,
            agent_name: "persist-agent",
            artifacts_already_published: false,
        })
        .await;

    let updated = persisted.expect("persisted");
    assert_eq!(updated.id, task_id);
    assert_eq!(updated.status.state, TaskState::Completed);
}

#[tokio::test]
async fn process_message_stream_provider_failure_emits_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().failing_stream());
    let processor = MessageProcessor::new(&pool, provider).expect("processor");

    let runtime = runtime_info("fail-agent");
    let request = request_context(&ctx, &session, &user, "fail-agent");
    let msg = user_message(&ctx, &task_id, "hi");

    let mut rx = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &msg,
            agent_runtime: &runtime,
            agent_name: "fail-agent",
            context: &request,
            task_id,
        })
        .await
        .expect("stream");

    let mut saw_error = false;
    while let Some(event) = rx.recv().await {
        if matches!(event, StreamEvent::Error(_)) {
            saw_error = true;
            break;
        }
        if matches!(event, StreamEvent::Complete { .. }) {
            break;
        }
    }
    assert!(
        saw_error,
        "expected an Error stream event on provider failure"
    );
}
