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
use systemprompt_test_mocks::recording_webhooks;
use tokio_util::sync::CancellationToken;

use super::a2a_helpers::{StubAiProvider, request_context, runtime_info};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

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
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let provider = Arc::new(StubAiProvider::new());
    MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor constructs");
}

#[tokio::test]
async fn process_message_stream_emits_text_and_complete() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["one ", "two"]));
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");

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
            cancel: CancellationToken::new(),
        })
        .await
        .expect("stream");

    let mut text = String::new();
    let mut completed = false;
    while let Some(event) = rx.events.recv().await {
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
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new());
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");

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

    let outcome = persisted.expect("persisted");
    assert_eq!(outcome.task.id, task_id);
    assert_eq!(outcome.task.status.state, TaskState::Completed);
    assert!(outcome.undelivered_broadcasts.is_empty());
}

#[tokio::test]
async fn cancelling_a_running_stream_emits_exactly_one_cancelled_event() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().with_stalled_stream());
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");

    let runtime = runtime_info("cancel-agent");
    let request = request_context(&ctx, &session, &user, "cancel-agent");
    let msg = user_message(&ctx, &task_id, "hi");
    let cancel = CancellationToken::new();

    let mut stream = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &msg,
            agent_runtime: &runtime,
            agent_name: "cancel-agent",
            context: &request,
            task_id,
            cancel: cancel.clone(),
        })
        .await
        .expect("stream");

    cancel.cancel();

    let mut terminal = Vec::new();
    while let Some(event) = stream.events.recv().await {
        match event {
            StreamEvent::Cancelled => terminal.push("cancelled"),
            StreamEvent::Complete { .. } => terminal.push("complete"),
            StreamEvent::Error(_) => terminal.push("error"),
            _ => {},
        }
    }
    assert_eq!(
        terminal,
        vec!["cancelled"],
        "one terminal event, and it is Cancelled"
    );
    assert!(stream.worker.is_finished() || stream.cancel.is_cancelled());
}

#[tokio::test]
async fn process_message_stream_provider_failure_emits_error() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new().failing_stream());
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");

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
            cancel: CancellationToken::new(),
        })
        .await
        .expect("stream");

    let mut saw_error = false;
    while let Some(event) = rx.events.recv().await {
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

#[tokio::test]
async fn configured_skills_are_injected_and_missing_optional_skills_do_not_abort_streaming() {
    use systemprompt_config::ProfileBootstrap;

    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.write().await;
    let skills_root = std::path::PathBuf::from(
        ProfileBootstrap::get()
            .expect("profile bootstrap")
            .paths
            .skills(),
    );
    let skill_directory = tempfile::Builder::new()
        .prefix("stream-skill-")
        .tempdir_in(&skills_root)
        .expect("create owned skill fixture");
    let skill_root = skill_directory.path();
    let skill_id = skill_root
        .file_name()
        .expect("skill directory name")
        .to_str()
        .expect("UTF-8 skill directory")
        .to_owned();
    std::fs::write(
        skill_root.join("config.yaml"),
        format!("id: {skill_id}\nname: Stream Skill\ndescription: stream injection fixture\n"),
    )
    .expect("write skill configuration");
    std::fs::write(
        skill_root.join("index.md"),
        "Use the retained-evidence procedure before answering.\n",
    )
    .expect("write skill instructions");

    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user, &session).await;
    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["done"]));
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider.clone(),
        recording_webhooks(),
    )
    .expect("processor");
    let mut runtime = runtime_info("skill-stream-agent");
    runtime.skills.include = vec![skill_id.clone(), "absent-optional-skill".to_owned()];
    let request = request_context(&ctx, &session, &user, "skill-stream-agent");
    let message = user_message(&ctx, &task_id, "answer with your configured procedure");

    let mut stream = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &message,
            agent_runtime: &runtime,
            agent_name: "skill-stream-agent",
            context: &request,
            task_id,
            cancel: CancellationToken::new(),
        })
        .await
        .expect("configured skill stream starts");
    while let Some(event) = stream.events.recv().await {
        if matches!(event, StreamEvent::Complete { .. }) {
            break;
        }
        if let StreamEvent::Error(error) = event {
            panic!("missing optional skill must not abort the stream: {error}");
        }
    }

    let requests = provider.seen_messages();
    assert_eq!(requests.len(), 1);
    let messages = &requests[0];
    assert_eq!(messages[0].role, systemprompt_models::MessageRole::System);
    assert!(messages[0].content.contains("# Your Skills"));
    assert!(
        messages[0]
            .content
            .contains(&format!("## {skill_id} Skill"))
    );
    assert!(
        messages[0]
            .content
            .contains("Use the retained-evidence procedure before answering.")
    );
    assert!(!messages[0].content.contains("absent-optional-skill"));
    assert_eq!(messages[1].content, "You are a test agent.");
    assert_eq!(
        messages.last().expect("user message").content,
        "answer with your configured procedure"
    );
}

#[tokio::test]
async fn partial_provider_stream_preserves_text_then_emits_one_error_without_completion() {
    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user, &session).await;
    let provider =
        Arc::new(StubAiProvider::new().with_partial_stream_failure("retained partial answer"));
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");
    let runtime = runtime_info("partial-failure-agent");
    let request = request_context(&ctx, &session, &user, "partial-failure-agent");
    let message = user_message(&ctx, &task_id, "stream then fail");

    let mut stream = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &message,
            agent_runtime: &runtime,
            agent_name: "partial-failure-agent",
            context: &request,
            task_id,
            cancel: CancellationToken::new(),
        })
        .await
        .expect("partial stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.events.recv().await {
        events.push(event);
    }

    let text_index = events
        .iter()
        .position(
            |event| matches!(event, StreamEvent::Text(text) if text == "retained partial answer"),
        )
        .unwrap_or_else(|| panic!("partial text event missing: {events:?}"));
    let errors = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Error(error) => Some(error.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "exactly one terminal error: {events:?}");
    assert!(
        errors[0].contains("stub partial stream failure"),
        "{errors:?}"
    );
    let error_index = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Error(_)))
        .expect("error event exists");
    assert!(
        text_index < error_index,
        "partial text must be delivered before the terminal error: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, StreamEvent::Complete { .. })),
        "a failed partial stream must not fabricate completion: {events:?}"
    );
}
#[tokio::test]
async fn planned_stream_synthesizes_tool_results_before_completing_with_the_final_summary() {
    use rmcp::model::{CallToolResult, ContentBlock};
    use serde_json::json;
    use systemprompt_models::ai::{PlannedToolCall, PlanningResult};

    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user, &session).await;

    let provider = Arc::new(
        StubAiProvider::new()
            .with_plan(PlanningResult::tool_calls(
                "inspect the record",
                vec![PlannedToolCall::new("lookup", json!({"id": 42}))],
            ))
            .with_tool_result(
                "lookup",
                CallToolResult::success(vec![ContentBlock::text("record 42 is active")]),
            )
            .with_generate("final summary after wrapper synthesis")
            .with_response("initial strategy synthesis"),
    );
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");
    let mut runtime = runtime_info("planned-stream-agent");
    runtime.mcp_servers.include = vec!["records".to_owned()];
    let request = request_context(&ctx, &session, &user, "planned-stream-agent");
    let message = user_message(&ctx, &task_id, "is record 42 active?");

    let mut stream = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &message,
            agent_runtime: &runtime,
            agent_name: "planned-stream-agent",
            context: &request,
            task_id,
            cancel: CancellationToken::new(),
        })
        .await
        .expect("planned stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.events.recv().await {
        events.push(event);
    }

    let initial = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Text(text) if text == "initial strategy synthesis"))
        .unwrap_or_else(|| panic!("strategy synthesis missing: {events:?}"));
    let final_text = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Text(text) if text == "final summary after wrapper synthesis"))
        .unwrap_or_else(|| panic!("wrapper synthesis missing: {events:?}"));
    let completed = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Complete { full_text, artifacts } if full_text == "final summary after wrapper synthesis" && artifacts.is_empty()))
        .unwrap_or_else(|| panic!("final completion missing: {events:?}"));
    assert!(
        initial < final_text && final_text < completed,
        "tool synthesis must finish before completion: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, StreamEvent::Error(_))),
        "a successful tool turn must not emit an error: {events:?}"
    );
}

#[tokio::test]
async fn planned_stream_reports_final_resynthesis_failure_without_completing() {
    use rmcp::model::{CallToolResult, ContentBlock};
    use serde_json::json;
    use systemprompt_models::ai::{PlannedToolCall, PlanningResult};

    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user, &session).await;
    let provider = Arc::new(
        StubAiProvider::new()
            .with_plan(PlanningResult::tool_calls(
                "inspect the record",
                vec![PlannedToolCall::new("lookup", json!({"id": 43}))],
            ))
            .with_tool_result(
                "lookup",
                CallToolResult::success(vec![ContentBlock::text("record 43 is inactive")]),
            )
            .failing_generate()
            .with_response("intermediate tool explanation"),
    );
    let processor = MessageProcessor::new(
        Arc::new(crate::repository::repos(&pool)),
        provider,
        recording_webhooks(),
    )
    .expect("processor");
    let mut runtime = runtime_info("planned-stream-agent");
    runtime.mcp_servers.include = vec!["records".to_owned()];
    let request = request_context(&ctx, &session, &user, "planned-stream-agent");
    let message = user_message(&ctx, &task_id, "is record 43 active?");

    let mut stream = processor
        .process_message_stream(ProcessMessageStreamParams {
            a2a_message: &message,
            agent_runtime: &runtime,
            agent_name: "planned-stream-agent",
            context: &request,
            task_id,
            cancel: CancellationToken::new(),
        })
        .await
        .expect("planned stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.events.recv().await {
        events.push(event);
    }
    let initial = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Text(text) if text == "intermediate tool explanation"))
        .unwrap_or_else(|| panic!("intermediate synthesis missing: {events:?}"));
    let failed = events
        .iter()
        .position(|event| matches!(event, StreamEvent::Error(error) if error.contains("stub generate failure")))
        .unwrap_or_else(|| panic!("final synthesis failure missing: {events:?}"));
    assert!(
        initial < failed,
        "failure must follow the intermediate result: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, StreamEvent::Complete { .. })),
        "failed resynthesis must not complete: {events:?}"
    );
}
