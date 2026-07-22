// Drives create_sse_stream_with_registry past agent resolution using an
// injected registry snapshot: the happy path streams model text and completes
// the persisted task; an injected registry-load failure marks the task failed
// and emits the registry JSON-RPC error; a failing model stream surfaces
// through the event loop and fails the task.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use systemprompt_agent::AgentError;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::streaming::{
    CreateSseStreamParams, create_sse_stream_with_registry,
};
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::ServicesConfig;

use super::a2a_helpers::{StubAiProvider, agent_config, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn registry_with(agent_name: &str) -> AgentRegistry {
    let mut agents = HashMap::new();
    agents.insert(agent_name.to_owned(), agent_config(agent_name));
    AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    })
}

fn message(ctx: &ContextId, task_id: &TaskId, text: &str) -> Message {
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

async fn collect_events(
    stream: impl futures::Stream<Item = axum::response::sse::Event> + Send,
) -> Vec<String> {
    let mut events = Vec::new();
    let mut stream = std::pin::pin!(stream);
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(15), stream.next()).await {
        events.push(format!("{event:?}"));
        if events.len() > 64 {
            break;
        }
    }
    events
}

async fn wait_for_state(
    repos_handle: &systemprompt_agent::repository::A2ARepositories,
    task_id: &TaskId,
    expected: TaskState,
) -> bool {
    for _ in 0..100 {
        if let Ok(Some(task)) = repos_handle.tasks.get_task(task_id).await
            && task.status.state == expected
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn run_stream_with_injected_registry_streams_text_and_completes_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing) = seed_context_and_task(&repos_handle, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["stream ", "done"]));
    let state = make_handler_state(&pool, provider, 4);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream_with_registry(
        CreateSseStreamParams {
            message: message(&ctx, &task_id, "run"),
            agent_name: "test_agent".to_owned(),
            state,
            request_id: RequestId::Number(11),
            context,
            callback_config: None,
        },
        Ok(registry_with("test_agent")),
    )
    .await
    .map_err(|_| ())
    .expect("permit available");

    let events = collect_events(stream).await;
    assert!(
        events.iter().any(|e| e.contains("stream ")),
        "expected streamed text frames, got {events:?}"
    );

    assert!(
        wait_for_state(&repos_handle, &task_id, TaskState::Completed).await,
        "task must reach Completed after the stream drains"
    );
}

#[tokio::test]
async fn run_stream_with_injected_registry_failure_fails_task_and_emits_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing) = seed_context_and_task(&repos_handle, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream_with_registry(
        CreateSseStreamParams {
            message: message(&ctx, &task_id, "run"),
            agent_name: "test_agent".to_owned(),
            state,
            request_id: RequestId::Number(12),
            context,
            callback_config: None,
        },
        Err(AgentError::Init("injected registry failure".to_owned())),
    )
    .await
    .map_err(|_| ())
    .expect("permit available");

    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| e.contains("Failed to load agent registry")),
        "expected registry-load error event, got {events:?}"
    );

    assert!(
        wait_for_state(&repos_handle, &task_id, TaskState::Failed).await,
        "task must be marked Failed when the registry cannot be loaded"
    );
}

#[tokio::test]
async fn run_stream_with_failing_model_stream_fails_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing) = seed_context_and_task(&repos_handle, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().failing_stream());
    let state = make_handler_state(&pool, provider, 4);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream_with_registry(
        CreateSseStreamParams {
            message: message(&ctx, &task_id, "run"),
            agent_name: "test_agent".to_owned(),
            state,
            request_id: RequestId::Number(13),
            context,
            callback_config: None,
        },
        Ok(registry_with("test_agent")),
    )
    .await
    .map_err(|_| ())
    .expect("permit available");

    let _events = collect_events(stream).await;

    assert!(
        wait_for_state(&repos_handle, &task_id, TaskState::Failed).await,
        "task must be marked Failed when the model stream errors"
    );
}
