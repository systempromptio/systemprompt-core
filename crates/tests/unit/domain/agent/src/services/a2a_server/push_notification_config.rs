// DB-backed tests for the A2A push-notification config handlers. Each test
// seeds a real user/session/context/task (the config table FKs to agent_tasks)
// and drives the set/get/list/delete handlers through the constructed
// `AgentHandlerState`. Get/list/delete on an unknown task exercise the
// empty-result and zero-deleted branches.

use std::sync::Arc;

use axum::extract::State;
use systemprompt_agent::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    PushNotificationConfig, SetTaskPushNotificationConfigRequest,
};
use systemprompt_agent::services::a2a_server::handlers::push_notification_config::{
    handle_delete_push_notification_config, handle_get_push_notification_config,
    handle_list_push_notification_configs, handle_set_push_notification_config,
};
use systemprompt_identifiers::TaskId;

use super::a2a_helpers::{make_handler_state, StubAiProvider};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn config(url: &str) -> PushNotificationConfig {
    PushNotificationConfig {
        endpoint: String::new(),
        headers: None,
        url: url.to_owned(),
        token: Some("secret".to_owned()),
        authentication: None,
    }
}

#[tokio::test]
async fn set_then_list_then_delete_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (_ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 4);

    let set = handle_set_push_notification_config(
        State(Arc::clone(&state)),
        SetTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
            config: config("https://example.invalid/hook"),
        },
    )
    .await;
    assert!(set.is_ok(), "set should succeed for a real task");

    let list =
        handle_list_push_notification_configs(State(Arc::clone(&state)), task_id.clone()).await;
    let (status, body) = list.expect("list ok");
    assert_eq!(status, axum::http::StatusCode::OK);
    let total = body.0["result"]["total"].as_u64().expect("total");
    assert!(total >= 1);

    let get = handle_get_push_notification_config(
        State(Arc::clone(&state)),
        GetTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
        },
    )
    .await;
    assert!(get.is_ok());

    let del = handle_delete_push_notification_config(
        State(Arc::clone(&state)),
        DeleteTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
        },
    )
    .await;
    let (_status, body) = del.expect("delete ok");
    let deleted = body.0["result"]["deleted"].as_u64().expect("deleted");
    assert!(deleted >= 1);
}

#[tokio::test]
async fn get_unknown_task_returns_empty_configs() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 4);

    let get = handle_get_push_notification_config(
        State(state),
        GetTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
        },
    )
    .await;
    let (status, body) = get.expect("get ok");
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(body.0["result"]["configs"].as_array().expect("array").is_empty());
}

#[tokio::test]
async fn delete_unknown_task_reports_zero_deleted() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 4);

    let del = handle_delete_push_notification_config(
        State(state),
        DeleteTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
        },
    )
    .await;
    let (_status, body) = del.expect("delete ok");
    assert_eq!(body.0["result"]["deleted"].as_u64(), Some(0));
}

#[tokio::test]
async fn set_for_unknown_task_fails_on_fk() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 4);

    let set = handle_set_push_notification_config(
        State(state),
        SetTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
            config: config("https://example.invalid/hook"),
        },
    )
    .await;
    assert!(set.is_err(), "FK violation should surface as an error tuple");
}
