// Drives the four push-notification config handlers against a pool whose every
// acquire fails, covering the repository-failure arms that a live pool never
// reaches: each must map to a JSON-RPC -32603 error tuple carrying the
// underlying failure in `data` rather than panicking or returning 200.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use systemprompt_agent::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    PushNotificationConfig, SetTaskPushNotificationConfigRequest,
};
use systemprompt_agent::services::a2a_server::handlers::push_notification_config::{
    handle_delete_push_notification_config, handle_get_push_notification_config,
    handle_list_push_notification_configs, handle_set_push_notification_config,
};
use systemprompt_identifiers::TaskId;
use systemprompt_test_fixtures::closed_db_pool;

use super::a2a_helpers::{StubAiProvider, make_handler_state};

fn assert_internal_rpc_error(status: StatusCode, body: &serde_json::Value, message: &str) {
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["error"]["code"], -32603);
    assert_eq!(body["error"]["message"], message);
    assert!(
        body["error"]["data"]
            .as_str()
            .is_some_and(|d| !d.is_empty()),
        "the underlying failure is reported in data, got {body}"
    );
}

#[tokio::test]
async fn set_on_a_dead_pool_reports_the_add_failure() {
    let pool = closed_db_pool().await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);

    let (status, body) = handle_set_push_notification_config(
        State(state),
        SetTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
            config: PushNotificationConfig {
                endpoint: String::new(),
                headers: None,
                url: "https://example.invalid/hook".to_owned(),
                token: None,
                authentication: None,
            },
        },
    )
    .await
    .expect_err("a dead pool cannot store a config");

    assert_internal_rpc_error(status, &body.0, "Failed to add push notification config");
}

#[tokio::test]
async fn get_on_a_dead_pool_reports_the_read_failure() {
    let pool = closed_db_pool().await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);

    let (status, body) = handle_get_push_notification_config(
        State(state),
        GetTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
        },
    )
    .await
    .expect_err("a dead pool cannot read configs");

    assert_internal_rpc_error(status, &body.0, "Failed to get push notification configs");
}

#[tokio::test]
async fn list_on_a_dead_pool_reports_the_read_failure() {
    let pool = closed_db_pool().await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);

    let (status, body) = handle_list_push_notification_configs(State(state), TaskId::generate())
        .await
        .expect_err("a dead pool cannot list configs");

    assert_internal_rpc_error(status, &body.0, "Failed to list push notification configs");
}

#[tokio::test]
async fn delete_on_a_dead_pool_reports_the_delete_failure() {
    let pool = closed_db_pool().await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);

    let (status, body) = handle_delete_push_notification_config(
        State(state),
        DeleteTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
        },
    )
    .await
    .expect_err("a dead pool cannot delete configs");

    assert_internal_rpc_error(
        status,
        &body.0,
        "Failed to delete push notification configs",
    );
}
