// Drives handle_agent_request through the non-streaming dispatch arms that the
// tasks/get tests in `request_handler` never reach: SendMessage end to end,
// CancelTask found and missing, the list-push-config arm that dispatch declines
// and non-streaming then rejects, and the unsupported-request-type fallthrough.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};
use systemprompt_agent::services::a2a_server::handlers::request::handle_agent_request;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, UserId};
use systemprompt_models::RequestContext;

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn rpc_request(context: RequestContext, body: &Value) -> Request {
    let mut request = Request::builder()
        .method("POST")
        .uri("/a2a")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    request.extensions_mut().insert(context);
    request
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("json body"))
}

fn send_message_params(context_id: &ContextId) -> Value {
    json!({
        "message": {
            "role": "ROLE_USER",
            "parts": [{"text": "hello agent"}],
            "messageId": MessageId::generate().as_str(),
            "contextId": context_id.as_str(),
            "metadata": null,
            "extensions": null
        }
    })
}

struct Fixture {
    context_id: ContextId,
    user_id: UserId,
    session_id: SessionId,
}

async fn fixture(pool: &systemprompt_database::DbPool) -> Fixture {
    let repos = repos(pool);
    let (user_id, session_id) = seed_user_and_session(pool).await;
    let (context_id, task_id) = seed_context_and_task(&repos, &user_id, &session_id).await;
    repos.tasks.delete_task(&task_id).await.ok();
    Fixture {
        context_id,
        user_id,
        session_id,
    }
}

#[tokio::test]
async fn send_message_dispatch_returns_a_task_for_the_seeded_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let f = fixture(&pool).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&f.context_id, &f.session_id, &f.user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "SendMessage",
        "params": send_message_params(&f.context_id),
        "id": 11
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], json!(11));
    assert!(
        body.get("result").is_some() || body.get("error").is_some(),
        "SendMessage produces a JSON-RPC result or a structured error, got {body}"
    );
    if let Some(result) = body.get("result") {
        assert_eq!(
            result["contextId"],
            json!(f.context_id.as_str()),
            "the task is bound to the request's context"
        );
    }
}

#[tokio::test]
async fn send_message_for_an_unknown_context_is_rejected_by_validation() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let f = fixture(&pool).await;
    let unknown = ContextId::generate();
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&f.context_id, &f.session_id, &f.user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "SendMessage",
        "params": send_message_params(&unknown),
        "id": 12
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["error"]["code"],
        json!(-32603),
        "a context the caller does not own is an internal-error envelope, got {body}"
    );
}

#[tokio::test]
async fn cancel_task_returns_a_canceled_task_bound_to_its_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&repos, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "CancelTask",
        "params": {"id": task_id.as_str()},
        "id": 13
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;
    repos.tasks.delete_task(&task_id).await.ok();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["id"], json!(task_id.as_str()));
    assert_eq!(body["result"]["contextId"], json!(context_id.as_str()));
    assert_eq!(
        body["result"]["status"]["state"],
        json!("TASK_STATE_CANCELED"),
        "cancel reports the canceled state, got {body}"
    );
}

#[tokio::test]
async fn cancel_task_for_an_unknown_id_is_a_jsonrpc_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&f.context_id, &f.session_id, &f.user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "CancelTask",
        "params": {"id": "no-such-task"},
        "id": 14
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], json!(-32603));
    assert!(
        body["error"]["data"]
            .as_str()
            .is_some_and(|d| d.contains("Task not found")),
        "the failure names the missing task, got {body}"
    );
}

#[tokio::test]
async fn list_push_notification_configs_is_declined_by_dispatch_and_rejected_downstream() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&repos, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "ListTaskPushNotificationConfigs",
        "params": {"task_id": task_id.as_str()},
        "id": 15
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;
    repos.tasks.delete_task(&task_id).await.ok();

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["error"]["data"]
            .as_str()
            .is_some_and(|d| d.contains("Push notification config requests")),
        "list is the one push verb dispatch declines, and non-streaming refuses it, got {body}"
    );
}

#[tokio::test]
async fn an_extended_card_request_falls_through_to_unsupported() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&f.context_id, &f.session_id, &f.user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "GetExtendedAgentCard",
        "params": {},
        "id": 16
    });
    let response = handle_agent_request(State(state), rpc_request(ctx, &payload))
        .await
        .into_response();
    let (status, body) = body_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body["error"]["data"]
            .as_str()
            .is_some_and(|d| d.contains("Unsupported request type")),
        "a parseable but unhandled request type is refused, got {body}"
    );
}
