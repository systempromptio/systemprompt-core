// Tests for the streaming event loop (`process_events`): fan-out of stream
// events to SSE frames, AG-UI webhook events (via a recording broadcaster
// installed through the `install_for_test` seam), and task-state updates.
// Covers the completion path (task completed + persisted), the failure path,
// and the tool-call / tool-result / execution-step broadcasts.

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use axum::response::sse::Event;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::a2a_server::processing::message::{
    MessageProcessor, StreamEvent,
};
use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    WebhookBroadcaster, WebhookError, install_for_test,
};
use systemprompt_agent::services::a2a_server::streaming::{ProcessEventsParams, process_events};
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId, UserId};
use systemprompt_models::{
    A2AEvent, AgUiEvent, CallToolResult, ExecutionStep, StepContent, StepId, StepStatus, ToolCall,
};
use tokio::sync::mpsc;

use super::a2a_helpers::{StubAiProvider, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

#[derive(Debug, Default)]
struct RecordingBroadcaster {
    agui: Mutex<Vec<String>>,
    a2a: Mutex<Vec<String>>,
}

#[async_trait]
impl WebhookBroadcaster for RecordingBroadcaster {
    async fn broadcast_agui(
        &self,
        _user_id: &UserId,
        event: AgUiEvent,
        _auth_token: &str,
    ) -> Result<usize, WebhookError> {
        let json = serde_json::to_string(&event).unwrap_or_default();
        self.agui.lock().expect("lock").push(json);
        Ok(1)
    }

    async fn broadcast_a2a(
        &self,
        _user_id: &UserId,
        event: A2AEvent,
        _auth_token: &str,
    ) -> Result<usize, WebhookError> {
        let json = serde_json::to_string(&event).unwrap_or_default();
        self.a2a.lock().expect("lock").push(json);
        Ok(1)
    }
}

fn recorder() -> &'static Arc<RecordingBroadcaster> {
    static RECORDER: OnceLock<Arc<RecordingBroadcaster>> = OnceLock::new();
    RECORDER.get_or_init(|| {
        let recorder = Arc::new(RecordingBroadcaster::default());
        install_for_test(Arc::clone(&recorder) as Arc<dyn WebhookBroadcaster>);
        recorder
    })
}

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
}

async fn spawn_loop() -> Option<Loop> {
    let pool = try_pool().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let processor =
        Arc::new(MessageProcessor::new(&pool, Arc::new(StubAiProvider::new())).expect("processor"));
    let task_repo = TaskRepository::new(&pool).expect("task repo");
    let request = request_context(&ctx, &session, &user, "loop-agent");

    let (sse_tx, sse_rx) = mpsc::channel::<Event>(64);
    let (event_tx, chunk_rx) = mpsc::channel::<StreamEvent>(64);

    let params = ProcessEventsParams {
        tx: sse_tx,
        chunk_rx,
        task_id: task_id.clone(),
        context_id: ctx.clone(),
        message_id: MessageId::generate(),
        original_message: user_message(&ctx, &task_id),
        agent_name: "loop-agent".to_owned(),
        context: request,
        task_repo,
        processor,
        request_id: RequestId::Number(1),
    };

    let handle = tokio::spawn(process_events(params));

    Some(Loop {
        event_tx,
        sse_rx,
        handle,
        task_id,
        pool,
    })
}

fn recorded_for(entries: &Mutex<Vec<String>>, task_id: &TaskId) -> Vec<String> {
    entries
        .lock()
        .expect("lock")
        .iter()
        .filter(|e| e.contains(task_id.as_str()))
        .cloned()
        .collect()
}

#[tokio::test]
async fn process_events_completion_path_persists_and_broadcasts() {
    let rec = recorder();
    let Some(mut ctx) = spawn_loop().await else {
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

    let mut sse_count = 0;
    while ctx.sse_rx.try_recv().is_ok() {
        sse_count += 1;
    }
    assert!(sse_count > 0, "expected SSE frames from the event loop");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Completed);

    let a2a = recorded_for(&rec.a2a, &ctx.task_id);
    assert!(
        a2a.iter().any(|e| e.contains("TASK_STATE_COMPLETED")),
        "expected an A2A completed broadcast, got: {a2a:?}"
    );
}

#[tokio::test]
async fn process_events_error_path_fails_task_and_broadcasts() {
    let rec = recorder();
    let Some(mut ctx) = spawn_loop().await else {
        return;
    };

    ctx.event_tx
        .send(StreamEvent::Error("model exploded".to_owned()))
        .await
        .expect("send error");

    ctx.handle.await.expect("loop finished");

    let mut saw_frame = false;
    while ctx.sse_rx.try_recv().is_ok() {
        saw_frame = true;
    }
    assert!(saw_frame, "expected SSE frames on the error path");

    let repos = repos(&ctx.pool);
    let stored = repos
        .tasks
        .get_task(&ctx.task_id)
        .await
        .expect("get task")
        .expect("task row");
    assert_eq!(stored.status.state, TaskState::Failed);

    let a2a = recorded_for(&rec.a2a, &ctx.task_id);
    assert!(
        a2a.iter().any(|e| e.contains("TASK_STATE_FAILED")),
        "expected an A2A failed broadcast, got: {a2a:?}"
    );
}

#[tokio::test]
async fn process_events_broadcasts_tool_and_step_events() {
    let rec = recorder();
    let Some(ctx) = spawn_loop().await else {
        return;
    };

    let call_id = AiToolCallId::new(uuid::Uuid::new_v4().to_string());
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
            call_id: call_id.to_string(),
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

    let agui: Vec<String> = rec.agui.lock().expect("lock").clone();
    assert!(
        agui.iter().any(|e| e.contains(call_id.as_str())),
        "expected AG-UI tool-call broadcasts for {call_id}"
    );
    assert!(
        agui.iter().any(|e| e.contains("step-1")),
        "expected an AG-UI execution-step broadcast"
    );
}
