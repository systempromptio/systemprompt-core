// Drives create_sse_stream through stream setup with a real seeded context:
// context validation passes, the initial task is persisted, and agent-runtime
// loading then fails against the empty fixture registry — the task is marked
// failed and an "Agent not found" JSON-RPC error event is emitted on the
// stream.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::streaming::{
    CreateSseStreamParams, create_sse_stream,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

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
    let Some(pool) = try_pool_or_skip().await else {
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
    let stored = stored.expect("initial task must have been persisted");
    assert_eq!(stored.id, task_id);
}

#[tokio::test]
async fn setup_without_task_id_mints_one_and_validates_context() {
    let Some(pool) = try_pool_or_skip().await else {
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

#[tokio::test]
async fn setup_with_unknown_context_emits_validation_error_and_persists_nothing() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let unknown_ctx = ContextId::generate();

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let context = request_context(&unknown_ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream(CreateSseStreamParams {
        message: message(&unknown_ctx, Some(task_id.clone())),
        agent_name: "test_agent".to_owned(),
        state,
        request_id: RequestId::Number(7),
        context,
    })
    .await
    .map_err(|_| ())
    .expect("permit available");

    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| e.contains("Context validation failed")),
        "expected context-validation error event, got {events:?}"
    );

    let stored = repos_handle.tasks.get_task(&task_id).await.expect("query");
    assert!(
        stored.is_none(),
        "no task may be persisted when context validation fails"
    );
}

#[tokio::test]
async fn invalid_service_agent_name_emits_invalid_params_before_task_persistence_or_dispatch() {
    let pool = try_pool_or_skip()
        .await
        .expect("agent streaming setup database fixture");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing_task) = seed_context_and_task(&repos_handle, &user, &session).await;
    let before = repos_handle
        .tasks
        .list_tasks_by_context(&ctx)
        .await
        .expect("tasks before invalid request")
        .len();
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider.clone(), 4);
    let context = request_context(&ctx, &session, &user, "valid_context_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx, Some(task_id.clone())),
        agent_name: "".to_owned(),
        state,
        request_id: RequestId::String("invalid-agent".to_owned()),
        context,
    })
    .await
    .map_err(|_| ())
    .expect("permit available");
    let events = collect_events(stream).await;
    assert!(
        events.iter().any(|event| {
            event.contains(r#"\"id\":\"invalid-agent\""#)
                && event.contains(r#"\"code\":-32602"#)
                && event.contains("Invalid agent name")
        }),
        "invalid service identity is returned on the correlated JSON-RPC stream: {events:?}"
    );
    assert!(
        repos_handle
            .tasks
            .get_task(&task_id)
            .await
            .expect("task lookup")
            .is_none(),
        "invalid service identity is rejected before task persistence"
    );
    assert_eq!(
        repos_handle
            .tasks
            .list_tasks_by_context(&ctx)
            .await
            .expect("tasks after invalid request")
            .len(),
        before
    );
    assert!(
        provider.seen_messages().is_empty(),
        "invalid service identity never reaches the AI provider"
    );
}

#[tokio::test]
async fn task_insert_failure_streams_internal_error_without_dispatch_or_partial_task() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = systemprompt_test_fixtures::DisposableDb::installed(
        "agent_stream_initial_task_insert_failure",
    )
    .await
    .expect("private agent database");
    let pool = database.pool().await.expect("private agent pool");
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _existing_task) = seed_context_and_task(&repos_handle, &user, &session).await;
    let before = repos_handle
        .tasks
        .list_tasks_by_context(&ctx)
        .await
        .expect("tasks before rejected insert")
        .len();
    let writer = pool.pool_arc().expect("private SQL pool");
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "CREATE FUNCTION reject_stream_task_insert() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture task insert rejection'; END $$; \
         CREATE TRIGGER reject_stream_task_insert BEFORE INSERT ON agent_tasks \
         FOR EACH ROW EXECUTE FUNCTION reject_stream_task_insert()",
    ))
    .execute(writer.as_ref())
    .await
    .expect("install private task-insert fault");
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider.clone(), 4);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let task_id = TaskId::generate();

    let stream = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx, Some(task_id.clone())),
        agent_name: "test_agent".to_owned(),
        state,
        request_id: RequestId::Number(41),
        context,
    })
    .await
    .map_err(|_| ())
    .expect("permit available");
    let events = collect_events(stream).await;
    assert!(
        events.iter().any(|event| {
            event.contains(r#"\"id\":41"#)
                && event.contains(r#"\"code\":-32603"#)
                && event.contains("Failed to create task")
                && event.contains("fixture task insert rejection")
        }),
        "task persistence failure is correlated and diagnosed on the stream: {events:?}"
    );
    assert!(
        repos_handle
            .tasks
            .get_task(&task_id)
            .await
            .expect("rejected task lookup")
            .is_none()
    );
    assert_eq!(
        repos_handle
            .tasks
            .list_tasks_by_context(&ctx)
            .await
            .expect("tasks after rejected insert")
            .len(),
        before,
        "failed initial persistence leaves no partial task"
    );
    assert!(
        provider.seen_messages().is_empty(),
        "a task that cannot be persisted never reaches the AI provider"
    );

    drop(writer);
    drop(repos_handle);
    drop(pool);
    database.drop_now().await;
}
