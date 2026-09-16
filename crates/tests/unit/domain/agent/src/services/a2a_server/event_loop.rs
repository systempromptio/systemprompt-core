// Tests for the streaming event loop (`process_events`): fan-out of stream
// events to SSE frames, AG-UI webhook events (via the recording broadcaster
// injected into the processor), and task-state updates. Covers the completion
// path (persisted first, then announced), the failure path, cancellation, and
// the tool-call / tool-result / execution-step broadcasts. Every path emits
// exactly one `final: true` status frame.

use std::sync::Arc;

use axum::response::sse::Event;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::a2a_server::processing::message::{
    MessageProcessor, MessageStream, StreamEvent,
};
use systemprompt_agent::services::a2a_server::streaming::{ProcessEventsParams, process_events};
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId};
use systemprompt_models::{
    CallToolResult, ExecutionStep, StepContent, StepId, StepStatus, ToolCall,
};
use systemprompt_test_mocks::{
    RecordedBroadcast, RecordingWebhookBroadcaster, arc_recording_broadcaster,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::a2a_helpers::{StubAiProvider, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

fn user_message(ctx: &ContextId, task_id: &TaskId) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "hi".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

struct Loop {
    event_tx: mpsc::Sender<StreamEvent>,
    sse_rx: mpsc::Receiver<Event>,
    handle: tokio::task::JoinHandle<()>,
    task_id: TaskId,
    pool: systemprompt_database::DbPool,
    rec: Arc<RecordingWebhookBroadcaster>,
}

// `agent_name` is the name the completion handler stamps into the task
// metadata; the request context always carries a valid `AgentName`, whose
// constructor rejects the empty string outright.
struct LoopSpec<'a> {
    agent_name: &'a str,
    persist_task_row: bool,
}

impl Default for LoopSpec<'_> {
    fn default() -> Self {
        Self {
            agent_name: "loop-agent",
            persist_task_row: true,
        }
    }
}

async fn spawn_loop_or_skip() -> Option<Loop> {
    spawn_loop_with_or_skip(LoopSpec::default()).await
}

async fn spawn_loop_with_or_skip(spec: LoopSpec<'_>) -> Option<Loop> {
    let pool = try_pool_or_skip().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, seeded_task_id) = seed_context_and_task(&repos, &user, &session).await;
    let task_id = if spec.persist_task_row {
        seeded_task_id
    } else {
        TaskId::generate()
    };

    let (broadcaster, rec) = arc_recording_broadcaster();
    let processor = Arc::new(
        MessageProcessor::new(
            Arc::new(crate::repository::repos(&pool)),
            Arc::new(StubAiProvider::new()),
            broadcaster,
        )
        .expect("processor"),
    );
    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    let request = request_context(&ctx, &session, &user, "loop-agent");

    let (sse_tx, sse_rx) = mpsc::channel::<Event>(64);
    let (event_tx, events) = mpsc::channel::<StreamEvent>(64);
    let stream = MessageStream {
        events,
        worker: tokio::spawn(async {}),
        cancel: CancellationToken::new(),
    };

    let params = ProcessEventsParams {
        tx: sse_tx,
        stream,
        task_id: task_id.clone(),
        context_id: ctx.clone(),
        message_id: MessageId::generate(),
        original_message: user_message(&ctx, &task_id),
        agent_name: spec.agent_name.to_owned(),
        context: request,
        task_repo,
        processor,
    };

    let handle = tokio::spawn(process_events(params));

    Some(Loop {
        event_tx,
        sse_rx,
        handle,
        task_id,
        pool,
        rec,
    })
}

fn a2a_for(rec: &RecordingWebhookBroadcaster, task_id: &TaskId) -> Vec<String> {
    rec.records()
        .into_iter()
        .filter_map(|r| match r {
            RecordedBroadcast::A2A { event, .. } => serde_json::to_string(&event).ok(),
            _ => None,
        })
        .filter(|e| e.contains(task_id.as_str()))
        .collect()
}

fn agui_all(rec: &RecordingWebhookBroadcaster) -> Vec<String> {
    rec.records()
        .into_iter()
        .filter_map(|r| match r {
            RecordedBroadcast::AgUi { event, .. } => serde_json::to_string(&event).ok(),
            _ => None,
        })
        .collect()
}

fn drain_frames(rx: &mut mpsc::Receiver<Event>) -> Vec<String> {
    let mut frames = Vec::new();
    while let Ok(frame) = rx.try_recv() {
        frames.push(format!("{frame:?}"));
    }
    frames
}

fn final_frames(frames: &[String]) -> Vec<&String> {
    frames
        .iter()
        .filter(|f| f.contains(r#"final\":true"#))
        .collect()
}

#[tokio::test]
async fn process_events_completion_path_persists_and_broadcasts() {
    let Some(mut ctx) = spawn_loop_or_skip().await else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Text("partial ".to_owned()))
        .await
        .expect("send text");
    ctx.event_tx
        .send(StreamEvent::Complete {
            full_text: "partial answer".to_owned(),
            artifacts: vec![],
        })
        .await
        .expect("send complete");

    ctx.handle.await.expect("loop finished");

    let frames = drain_frames(&mut ctx.sse_rx);
    assert!(
        !frames.is_empty(),
        "expected SSE frames from the event loop"
    );
    let finals = final_frames(&frames);
    assert_eq!(finals.len(), 1, "exactly one final frame: {frames:?}");
    assert!(finals[0].contains("TASK_STATE_COMPLETED"), "{finals:?}");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Completed);
    assert!(
        stored.history.as_ref().is_some_and(|h| h.len() >= 2),
        "the completed task carries its persisted messages: {:?}",
        stored.history
    );

    let a2a = a2a_for(&ctx.rec, &ctx.task_id);
    assert_eq!(
        a2a.iter()
            .filter(|e| e.contains("TASK_STATE_COMPLETED"))
            .count(),
        1,
        "one A2A completed broadcast, got: {a2a:?}"
    );
}

#[tokio::test]
async fn process_events_error_path_fails_task_and_broadcasts() {
    let Some(mut ctx) = spawn_loop_or_skip().await else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Error("model exploded".to_owned()))
        .await
        .expect("send error");

    ctx.handle.await.expect("loop finished");

    let frames = drain_frames(&mut ctx.sse_rx);
    let finals = final_frames(&frames);
    assert_eq!(finals.len(), 1, "exactly one final frame: {frames:?}");
    assert!(finals[0].contains("TASK_STATE_FAILED"), "{finals:?}");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Failed);

    let a2a = a2a_for(&ctx.rec, &ctx.task_id);
    assert_eq!(
        a2a.iter()
            .filter(|e| e.contains("TASK_STATE_FAILED"))
            .count(),
        1,
        "one A2A failed broadcast, got: {a2a:?}"
    );
}

#[tokio::test]
async fn process_events_cancelled_path_marks_task_canceled_with_one_final_frame() {
    let Some(mut ctx) = spawn_loop_or_skip().await else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Text("part".to_owned()))
        .await
        .expect("send text");
    ctx.event_tx
        .send(StreamEvent::Cancelled)
        .await
        .expect("send cancelled");

    ctx.handle.await.expect("loop finished");

    let frames = drain_frames(&mut ctx.sse_rx);
    let finals = final_frames(&frames);
    assert_eq!(finals.len(), 1, "exactly one final frame: {frames:?}");
    assert!(finals[0].contains("TASK_STATE_CANCELED"), "{finals:?}");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Canceled);

    let agui = agui_all(&ctx.rec);
    assert!(
        agui.iter().any(|e| e.contains("TASK_CANCELLED")),
        "cancellation is reported to AG-UI: {agui:?}"
    );
}

#[tokio::test]
async fn process_events_broadcasts_tool_and_step_events() {
    let Some(ctx) = spawn_loop_or_skip().await else {
        return;
    };

    let call_id = AiToolCallId::generate();
    ctx.event_tx
        .send(StreamEvent::ToolCallStarted(ToolCall {
            ai_tool_call_id: call_id.clone(),
            name: "search".to_owned(),
            arguments: serde_json::json!({"q": "answer"}),
        }))
        .await
        .expect("send tool call");
    ctx.event_tx
        .send(StreamEvent::ToolResult {
            ai_tool_call_id: call_id.clone(),
            result: CallToolResult::success(vec![]),
        })
        .await
        .expect("send tool result");
    ctx.event_tx
        .send(StreamEvent::ExecutionStepUpdate {
            step: ExecutionStep {
                step_id: StepId("step-1".to_owned()),
                task_id: ctx.task_id.clone(),
                status: StepStatus::Completed,
                started_at: chrono::Utc::now(),
                completed_at: Some(chrono::Utc::now()),
                duration_ms: Some(5),
                error_message: None,
                content: StepContent::Completion,
            },
        })
        .await
        .expect("send step");
    ctx.event_tx
        .send(StreamEvent::Complete {
            full_text: "done".to_owned(),
            artifacts: vec![],
        })
        .await
        .expect("send complete");

    ctx.handle.await.expect("loop finished");

    let agui = agui_all(&ctx.rec);
    assert!(
        agui.iter().any(|e| e.contains(call_id.as_str())),
        "expected AG-UI tool-call broadcasts for {call_id}"
    );
    assert!(
        agui.iter().any(|e| e.contains("step-1")),
        "expected an AG-UI execution-step broadcast"
    );
}

#[tokio::test]
async fn completion_with_an_empty_agent_name_fails_the_task_before_persistence() {
    let Some(mut ctx) = spawn_loop_with_or_skip(LoopSpec {
        agent_name: "",
        ..LoopSpec::default()
    })
    .await
    else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Complete {
            full_text: "answer".to_owned(),
            artifacts: vec![],
        })
        .await
        .expect("send complete");
    ctx.handle.await.expect("loop finished");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(
        stored.status.state,
        TaskState::Failed,
        "nothing marks the task completed before its messages are committed"
    );

    let frames = drain_frames(&mut ctx.sse_rx);
    let finals = final_frames(&frames);
    assert_eq!(finals.len(), 1, "exactly one final frame: {frames:?}");
    assert!(finals[0].contains("TASK_STATE_FAILED"), "{finals:?}");

    let agui = agui_all(&ctx.rec);
    assert!(
        agui.iter().any(|e| e.contains("METADATA_ERROR")),
        "an unusable agent name is reported as a RUN_ERROR"
    );
    assert!(
        !agui
            .iter()
            .any(|e| e.contains("RUN_FINISHED") && e.contains(ctx.task_id.as_str())),
        "the success fan-out is skipped when metadata cannot be built"
    );
}

#[tokio::test]
async fn completion_of_an_unpersisted_task_reports_a_persistence_error() {
    let Some(mut ctx) = spawn_loop_with_or_skip(LoopSpec {
        persist_task_row: false,
        ..LoopSpec::default()
    })
    .await
    else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Complete {
            full_text: "answer".to_owned(),
            artifacts: vec![],
        })
        .await
        .expect("send complete");
    ctx.handle.await.expect("loop finished");

    let repos = repos(&ctx.pool);
    assert!(
        repos
            .tasks
            .get_task(&ctx.task_id)
            .await
            .expect("get task")
            .is_none(),
        "the task was never persisted, so nothing is written back"
    );

    let frames = drain_frames(&mut ctx.sse_rx);
    let finals = final_frames(&frames);
    assert_eq!(finals.len(), 1, "exactly one final frame: {frames:?}");

    let agui = agui_all(&ctx.rec);
    assert!(
        agui.iter().any(|e| e.contains("PERSISTENCE_ERROR")),
        "a failed write is reported as a RUN_ERROR"
    );
    assert!(
        !agui
            .iter()
            .any(|e| e.contains("RUN_FINISHED") && e.contains(ctx.task_id.as_str())),
        "the success fan-out is skipped when persistence fails"
    );
}
