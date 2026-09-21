// DB-backed tests for the non-streaming message pipeline
// (`MessageProcessor::handle_message_with_runtime`): full run against a
// stubbed provider (task persisted, completed, response text captured),
// task-id reuse from the inbound message, and the context-validation
// failure path.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::ActiveTasks;
use systemprompt_agent::services::a2a_server::processing::message::{
    HandleMessageParams, MessageProcessor,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_test_mocks::recording_webhooks;

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

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
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["It is ", "42."]));
    let processor = MessageProcessor::new(Arc::new(repos.clone()), provider, recording_webhooks())
        .expect("processor");

    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&ctx, None, "what is the answer?");

    let task = processor
        .handle_message_with_runtime(HandleMessageParams {
            message: msg,
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
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
    let history = stored.history.expect("completed conversation history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, MessageRole::User);
    assert_eq!(history[1].role, MessageRole::Agent);
    assert!(
        matches!(history[0].parts.as_slice(), [Part::Text(text)] if text.text == "what is the answer?")
    );
    assert!(matches!(history[1].parts.as_slice(), [Part::Text(text)] if text.text == "It is 42."));
    assert_eq!(history[0].task_id.as_ref(), Some(&task.id));
    assert_eq!(history[1].task_id.as_ref(), Some(&task.id));
}

#[tokio::test]
async fn handle_message_with_runtime_reuses_inbound_task_id() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;
    let client_task_id = TaskId::generate();

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["continuing"]));
    let processor = MessageProcessor::new(Arc::new(repos.clone()), provider, recording_webhooks())
        .expect("processor");

    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&ctx, Some(client_task_id.clone()), "more");

    let task = processor
        .handle_message_with_runtime(HandleMessageParams {
            message: msg,
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect("handled");

    assert_eq!(task.id, client_task_id);
}

#[tokio::test]
async fn handle_message_with_runtime_surfaces_model_stream_failure() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;
    let client_task_id = TaskId::generate();

    let provider = Arc::new(StubAiProvider::new().failing_stream());
    let processor = MessageProcessor::new(Arc::new(repos.clone()), provider, recording_webhooks())
        .expect("processor");

    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&ctx, Some(client_task_id.clone()), "boom");

    processor
        .handle_message_with_runtime(HandleMessageParams {
            message: msg,
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect_err("failing model stream must propagate as an error");

    let stored = repos
        .tasks
        .get_task(&client_task_id)
        .await
        .expect("get task")
        .expect("initial task must have been persisted before the failure");
    assert_eq!(stored.status.state, TaskState::Working);
    assert!(stored.history.as_ref().is_none_or(Vec::is_empty));
    let database = pool.pool_arc().expect("pool");
    let persisted = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT status, error_message FROM agent_tasks WHERE task_id = $1",
    )
    .bind(client_task_id.as_str())
    .fetch_one(database.as_ref())
    .await
    .expect("persisted failure boundary");
    assert_eq!(persisted, ("TASK_STATE_WORKING".to_owned(), None));
}

#[tokio::test]
async fn cancellation_marks_nonstream_task_canceled_without_agent_response() {
    let pool = try_pool_or_skip()
        .await
        .expect("agent cancellation fixture database");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;
    let task_id = TaskId::generate();
    let active = ActiveTasks::default();
    let active_for_run = active.clone();
    let processor = MessageProcessor::new(
        Arc::new(repos.clone()),
        Arc::new(StubAiProvider::new().with_stalled_stream()),
        recording_webhooks(),
    )
    .expect("processor");
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&ctx, &session, &user, "nonstream-agent");
    let message = user_message(&ctx, Some(task_id.clone()), "cancel this run");

    let handle = tokio::spawn(async move {
        processor
            .handle_message_with_runtime(HandleMessageParams {
                message,
                agent_runtime: &runtime,
                agent_name: "nonstream-agent",
                context: &request,
                active_tasks: &active_for_run,
            })
            .await
    });
    for _ in 0..80 {
        if active.is_running(&task_id) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        active.cancel(&task_id),
        "registered run accepts cancellation"
    );
    let error = handle
        .await
        .expect("handler join")
        .expect_err("cancelled run");
    assert!(matches!(
        error,
        systemprompt_agent::services::shared::AgentServiceError::TaskCancelled
    ));
    assert!(
        active
            .wait_until_finished(&task_id, std::time::Duration::from_secs(2))
            .await
    );

    let stored = repos.tasks.get_task(&task_id).await.unwrap().unwrap();
    assert_eq!(stored.status.state, TaskState::Canceled);
    assert!(stored.history.as_ref().is_none_or(Vec::is_empty));
}

#[tokio::test]
async fn handle_message_with_runtime_rejects_unowned_context() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let (user, session) = seed_user_and_session(&pool).await;

    let repos = repos(&pool);
    let provider = Arc::new(StubAiProvider::new());
    let processor = MessageProcessor::new(Arc::new(repos.clone()), provider, recording_webhooks())
        .expect("processor");

    let foreign_ctx = ContextId::generate();
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&foreign_ctx, &session, &user, "nonstream-agent");
    let msg = user_message(&foreign_ctx, None, "hi");

    let err = processor
        .handle_message_with_runtime(HandleMessageParams {
            message: msg,
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect_err("unowned context must fail");
    assert!(err.to_string().contains("Context validation failed"));
}
#[tokio::test]
async fn completed_message_write_failure_marks_task_failed_without_partial_history() {
    use systemprompt_test_fixtures::DisposableDb;

    let database = DisposableDb::installed("agent_message_persist_failure")
        .await
        .expect("isolated agent database");
    let pool = database.pool().await.expect("agent database pool");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context_id, _) = seed_context_and_task(&repositories, &user, &session).await;
    let task_id = TaskId::generate();
    let raw = pool.write_pool_arc().expect("agent write pool");

    sqlx::query(
        "CREATE FUNCTION reject_completed_agent_message() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.role = 'agent' THEN RAISE EXCEPTION 'fixture agent message rejection'; \
         END IF; RETURN NEW; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("create message failure function");
    sqlx::query(
        "CREATE TRIGGER reject_completed_agent_message BEFORE INSERT ON task_messages \
         FOR EACH ROW EXECUTE FUNCTION reject_completed_agent_message()",
    )
    .execute(raw.as_ref())
    .await
    .expect("create message failure trigger");

    let processor = MessageProcessor::new(
        Arc::new(repositories.clone()),
        Arc::new(StubAiProvider::new().with_text_stream(&["completed answer"])),
        recording_webhooks(),
    )
    .expect("message processor");
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&context_id, &session, &user, "nonstream-agent");
    let message = user_message(
        &context_id,
        Some(task_id.clone()),
        "persist this conversation",
    );

    let error = processor
        .handle_message_with_runtime(HandleMessageParams {
            message,
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect_err("late agent-message write must fail completion");
    assert!(
        error
            .to_string()
            .contains("fixture agent message rejection"),
        "database diagnosis reaches the caller: {error}"
    );

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM agent_tasks WHERE task_id = $1")
            .bind(task_id.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("failed task row persists");
    assert_eq!(row.0, "TASK_STATE_FAILED");
    assert!(
        row.1
            .as_deref()
            .is_some_and(|message| message.contains("fixture agent message rejection")),
        "terminal task state retains the persistence diagnosis: {row:?}"
    );
    let message_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM task_messages WHERE task_id = $1")
            .bind(task_id.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("count task messages");
    assert_eq!(
        message_count, 0,
        "the completion transaction must not leave only the user half of history"
    );

    drop(processor);
    drop(repositories);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn working_transition_failure_leaves_submitted_task_without_starting_history() {
    use systemprompt_test_fixtures::DisposableDb;

    let database = DisposableDb::installed("agent_working_transition_failure")
        .await
        .expect("isolated agent database");
    let pool = database.pool().await.expect("agent database pool");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context_id, _) = seed_context_and_task(&repositories, &user, &session).await;
    let task_id = TaskId::generate();
    let raw = pool.write_pool_arc().expect("agent write pool");

    sqlx::query(
        "CREATE FUNCTION reject_working_transition() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.status = 'TASK_STATE_WORKING' THEN \
         RAISE EXCEPTION 'fixture working transition rejection'; END IF; RETURN NEW; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("create transition failure function");
    sqlx::query(
        "CREATE TRIGGER reject_working_transition BEFORE UPDATE OF status ON agent_tasks \
         FOR EACH ROW EXECUTE FUNCTION reject_working_transition()",
    )
    .execute(raw.as_ref())
    .await
    .expect("create transition failure trigger");

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["must not run"]));
    let processor = MessageProcessor::new(
        Arc::new(repositories.clone()),
        provider.clone(),
        recording_webhooks(),
    )
    .expect("message processor");
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&context_id, &session, &user, "nonstream-agent");

    let error = processor
        .handle_message_with_runtime(HandleMessageParams {
            message: user_message(&context_id, Some(task_id.clone()), "start this task"),
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect_err("working-state transition must fail before model dispatch");
    assert!(
        error
            .to_string()
            .contains("fixture working transition rejection"),
        "transition diagnosis reaches the caller: {error}"
    );

    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM agent_tasks WHERE task_id = $1")
            .bind(task_id.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("submitted task remains observable");
    assert_eq!(row, ("TASK_STATE_SUBMITTED".to_owned(), None));
    let message_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM task_messages WHERE task_id = $1")
            .bind(task_id.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("count task messages");
    assert_eq!(message_count, 0, "model/history pipeline never begins");
    assert!(
        provider.seen_messages().is_empty(),
        "the provider must not receive a request after the durable working transition fails"
    );

    drop(processor);
    drop(repositories);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
#[tokio::test]
async fn context_read_failure_prevents_task_creation_and_provider_dispatch() {
    use systemprompt_test_fixtures::DisposableDb;

    let database = DisposableDb::installed("agent_context_read_failure")
        .await
        .expect("isolated agent database");
    let pool = database.pool().await.expect("agent database pool");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context_id, _) = seed_context_and_task(&repositories, &user, &session).await;
    let task_id = TaskId::generate();
    let raw = pool.write_pool_arc().expect("agent write pool");

    sqlx::query("ALTER TABLE user_contexts RENAME TO unavailable_user_contexts")
        .execute(raw.as_ref())
        .await
        .expect("make context storage unavailable");

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["must not run"]));
    let processor = MessageProcessor::new(
        Arc::new(repositories.clone()),
        provider.clone(),
        recording_webhooks(),
    )
    .expect("message processor");
    let runtime = runtime_info("nonstream-agent");
    let request = request_context(&context_id, &session, &user, "nonstream-agent");

    let error = processor
        .handle_message_with_runtime(HandleMessageParams {
            message: user_message(&context_id, Some(task_id.clone()), "read this context"),
            agent_runtime: &runtime,
            agent_name: "nonstream-agent",
            context: &request,
            active_tasks: &ActiveTasks::default(),
        })
        .await
        .expect_err("unavailable context storage must fail closed");
    let diagnosis = error.to_string();
    assert!(
        diagnosis.contains("Context validation failed")
            && diagnosis.contains("user_contexts")
            && diagnosis.contains("does not exist"),
        "context storage diagnosis reaches the caller: {diagnosis}"
    );

    let task_count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_tasks WHERE task_id = $1")
        .bind(task_id.as_str())
        .fetch_one(raw.as_ref())
        .await
        .expect("count tasks after context failure");
    assert_eq!(task_count, 0, "context validation precedes task creation");
    assert!(
        provider.seen_messages().is_empty(),
        "context validation failure must prevent provider dispatch"
    );

    drop(processor);
    drop(repositories);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
