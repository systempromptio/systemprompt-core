// Tests for the streaming lifecycle helpers: emit_run_started (task moves to
// Working, a status SSE frame is emitted, A2A + RUN_STARTED webhooks fire) and
// handle_stream_creation_error (task marked failed with the error message and
// a RUN_ERROR webhook). Webhook traffic is captured by the recording
// broadcaster injected through `WebhookContext`.


use axum::response::sse::Event;
use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::WebhookContext;
use systemprompt_agent::services::a2a_server::streaming::{
    EmitRunStartedParams, emit_run_started, handle_stream_creation_error,
};
use systemprompt_agent::services::shared::AgentServiceError;
use systemprompt_identifiers::TaskId;
use systemprompt_test_mocks::{
    RecordedBroadcast, RecordingWebhookBroadcaster, arc_recording_broadcaster,
};
use tokio::sync::mpsc;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

fn agui_for(rec: &RecordingWebhookBroadcaster, task_id: &TaskId) -> Vec<String> {
    rec.records()
        .into_iter()
        .filter_map(|r| match r {
            RecordedBroadcast::AgUi { event, .. } => serde_json::to_string(&event).ok(),
            _ => None,
        })
        .filter(|e| e.contains(task_id.as_str()))
        .collect()
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

#[tokio::test]
async fn emit_run_started_moves_task_to_working_and_emits_status_frame() {
    let (broadcaster, rec) = arc_recording_broadcaster();
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    let webhook_context = WebhookContext::new(broadcaster, user_id.clone(), "tok");
    let (tx, mut rx) = mpsc::channel::<Event>(8);

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
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
    assert!(rendered.contains("TASK_STATE_WORKING"));
    assert!(rendered.contains(task_id.as_str()));
    assert!(rendered.contains(r#"final\":false"#));

    let a2a = a2a_for(&rec, &task_id);
    assert!(!a2a.is_empty());
    let agui = agui_for(&rec, &task_id);
    assert!(agui.iter().any(|e| e.contains("RUN_STARTED")));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn emit_run_started_still_updates_task_when_sse_channel_closed() {
    let (broadcaster, _rec) = arc_recording_broadcaster();
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    let webhook_context = WebhookContext::new(broadcaster, user_id.clone(), "tok");
    let (tx, rx) = mpsc::channel::<Event>(1);
    drop(rx);

    emit_run_started(EmitRunStartedParams {
        tx: &tx,
        webhook_context: &webhook_context,
        context_id: &context_id,
        task_id: &task_id,
        task_repo: &task_repo,
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
    let (broadcaster, rec) = arc_recording_broadcaster();
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    let webhook_context = WebhookContext::new(broadcaster, user_id.clone(), "tok");

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

    let agui = agui_for(&rec, &task_id);
    let all_agui: Vec<String> = rec
        .records()
        .into_iter()
        .filter_map(|r| match r {
            RecordedBroadcast::AgUi { event, .. } => serde_json::to_string(&event).ok(),
            _ => None,
        })
        .collect();
    assert!(
        all_agui
            .iter()
            .any(|e| e.contains("STREAM_CREATION_ERROR") && e.contains("upstream refused")),
        "{agui:?}"
    );

    r.tasks.delete_task(&task_id).await.ok();
}
