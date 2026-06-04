//! Validates the `WebhookBroadcaster` injection seam — `install_for_test`
//! swaps in a recording fake and the free `broadcast_agui_event` /
//! `broadcast_a2a_event` entry points dispatch through it. This is the
//! contract every deeper `complete_handler` / `message_handler` / `skills`
//! call relies on for in-process A2A streaming coverage.

use std::sync::OnceLock;

use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    broadcast_a2a_event, broadcast_agui_event, install_for_test,
};
use systemprompt_identifiers::{ContextId, SkillId, TaskId, UserId};
use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder};
use systemprompt_test_mocks::{
    RecordedBroadcast, RecordingWebhookBroadcaster, arc_recording_broadcaster,
};

static SHARED_RECORDER: OnceLock<std::sync::Arc<RecordingWebhookBroadcaster>> = OnceLock::new();

fn install_shared_recorder() -> std::sync::Arc<RecordingWebhookBroadcaster> {
    SHARED_RECORDER
        .get_or_init(|| {
            let (dyn_arc, inner) = arc_recording_broadcaster();
            install_for_test(dyn_arc);
            inner
        })
        .clone()
}

fn sample_agui_event() -> systemprompt_models::AgUiEvent {
    AgUiEventBuilder::skill_loaded(
        SkillId::new("skill-x"),
        "skill-x".to_owned(),
        Some("desc".to_owned()),
        None,
    )
}

#[tokio::test]
async fn broadcast_agui_routes_through_installed_broadcaster() {
    let recorder = install_shared_recorder();
    let start = recorder.record_count();
    let user = UserId::new("user-agui");
    let result = broadcast_agui_event(&user, sample_agui_event(), "token-agui").await;
    assert!(result.is_ok(), "broadcast failed: {result:?}");
    let records = recorder.records();
    let recent = &records[start..];
    assert!(recent.iter().any(|r| matches!(
        r,
        RecordedBroadcast::AgUi { user_id, auth_token, .. }
            if user_id == &user && auth_token == "token-agui"
    )));
}

#[tokio::test]
async fn broadcast_a2a_routes_through_installed_broadcaster() {
    let recorder = install_shared_recorder();
    let start = recorder.record_count();
    let user = UserId::new("user-a2a");
    let event = A2AEventBuilder::task_submitted(
        TaskId::new("task-1"),
        ContextId::generate(),
        "test-agent".to_owned(),
        None,
    );
    let result = broadcast_a2a_event(&user, event, "token-a2a").await;
    assert!(result.is_ok(), "broadcast failed: {result:?}");
    let records = recorder.records();
    let recent = &records[start..];
    assert!(recent.iter().any(|r| matches!(
        r,
        RecordedBroadcast::A2A { user_id, auth_token, .. }
            if user_id == &user && auth_token == "token-a2a"
    )));
}

#[tokio::test]
async fn broadcast_returns_recorder_connection_count() {
    install_shared_recorder();
    let user = UserId::new("user-count");
    let count = broadcast_agui_event(&user, sample_agui_event(), "tok")
        .await
        .expect("ok");
    assert_eq!(count, 1);
}
