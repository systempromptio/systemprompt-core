// Tests for the streaming lifecycle helpers: emit_run_started (task moves to
// Working, a status SSE frame is emitted, A2A + RUN_STARTED webhooks fire) and
// handle_stream_creation_error (task marked failed with the error message and
// a RUN_ERROR webhook). Webhook traffic is captured via the install_for_test
// recording seam shared with the event-loop tests.

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use axum::response::sse::Event;
use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    WebhookBroadcaster, WebhookContext, WebhookError, install_for_test,
};
use systemprompt_agent::services::a2a_server::streaming::{
    EmitRunStartedParams, emit_run_started, handle_stream_creation_error,
};
use systemprompt_agent::services::shared::AgentServiceError;
use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent};
use tokio::sync::mpsc;

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
async fn emit_run_started_moves_task_to_working_and_emits_status_frame() {
    let rec = recorder();
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool).expect("task repo");
    let webhook_context = WebhookContext::new(user_id.clone(), "tok");
    let (tx, mut rx) = mpsc::channel::<Event>(8);
    let request_id = RequestId::Number(7);

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
        request_id: &request_id,
    })
    .await;

    let task = task_repo
        .get_task(&task_id)
        .await
        .expect("get task")
        .expect("task present");
    assert_eq!(task.status.state, TaskState::Working);

    let frame = rx.try_recv().expect("status frame emitted");
    let rendered = format!("{frame:?}");
    assert!(rendered.contains("status-update"));
    assert!(rendered.contains("working"));
    assert!(rendered.contains(task_id.as_str()));
    assert!(rendered.contains(r#"final\":false"#));

    let a2a = recorded_for(&rec.a2a, &task_id);
    assert!(!a2a.is_empty());
    let agui = recorded_for(&rec.agui, &task_id);
    assert!(agui.iter().any(|e| e.contains("RUN_STARTED")));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn emit_run_started_still_updates_task_when_sse_channel_closed() {
    let _rec = recorder();
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool).expect("task repo");
    let webhook_context = WebhookContext::new(user_id.clone(), "tok");
    let (tx, rx) = mpsc::channel::<Event>(1);
    drop(rx);
    let request_id = RequestId::Number(8);

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
        request_id: &request_id,
    })
    .await;

    let task = task_repo
        .get_task(&task_id)
        .await
        .expect("get task")
        .expect("task present");
    assert_eq!(task.status.state, TaskState::Working);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn stream_creation_error_marks_task_failed_and_broadcasts_run_error() {
    let rec = recorder();
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool).expect("task repo");
    let webhook_context = WebhookContext::new(user_id.clone(), "tok");

    handle_stream_creation_error(
        &webhook_context,
        AgentServiceError::Internal("upstream refused".to_owned()),
        &task_id,
        &context_id,
        &task_repo,
    )
    .await;

    let task = task_repo
        .get_task(&task_id)
        .await
        .expect("get task")
        .expect("task present");
    assert_eq!(task.status.state, TaskState::Failed);

    let agui = rec.agui.lock().expect("lock");
    assert!(
        agui.iter()
            .any(|e| e.contains("STREAM_CREATION_ERROR") && e.contains("upstream refused"))
    );

    r.tasks.delete_task(&task_id).await.ok();
}
