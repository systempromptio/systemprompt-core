// Tests for the top-level A2A JSON-RPC entry point handle_agent_request:
// missing-context guard, JSON / JSON-RPC parse failures, the OAuth gate, and
// full dispatch of tasks/get through the non-streaming path.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_agent::services::a2a_server::handlers::request::handle_agent_request;
use systemprompt_models::RequestContext;

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

async fn call(
    state: Arc<AgentHandlerState>,
    context: Option<RequestContext>,
    body: &str,
) -> axum::response::Response {
    handle_agent_request(
        State(state),
        context.map(Extension),
        HeaderMap::new(),
        Bytes::from(body.to_owned()),
    )
    .await
    .into_response()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let value: Value = serde_json::from_slice(&bytes).expect("json body");
    (status, value)
}

#[tokio::test]
async fn missing_request_context_returns_internal_error() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);

    let response = call(state, None, "{}").await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], json!(-32603));
}

#[tokio::test]
async fn invalid_json_body_returns_parse_error() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let response = call(state, Some(ctx), "{not json").await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], json!(-32700));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn non_jsonrpc_payload_returns_invalid_request() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let response = call(state, Some(ctx), r#"{"hello": "world"}"#).await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], json!(-32600));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn oauth_required_without_bearer_token_is_unauthorized() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    state.config.write().await.oauth.required = true;
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "GetTask",
        "params": {"id": task_id.as_str()},
        "id": 1
    });
    let response = call(state, Some(ctx), &payload.to_string()).await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["message"], json!("Unauthorized"));
    assert!(
        body["error"]["data"]["reason"]
            .as_str()
            .is_some_and(|m| m.contains("Bearer token required"))
    );

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_task_dispatch_returns_task_result() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "GetTask",
        "params": {"id": task_id.as_str()},
        "id": 42
    });
    let response = call(state, Some(ctx), &payload.to_string()).await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], json!(42));
    assert_eq!(body["result"]["id"], json!(task_id.as_str()));
    assert_eq!(body["result"]["contextId"], json!(context_id.as_str()));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_task_for_unknown_id_returns_jsonrpc_error() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&context_id, &session_id, &user_id, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "GetTask",
        "params": {"id": "no-such-task"},
        "id": 5
    });
    let response = call(state, Some(ctx), &payload.to_string()).await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], json!(-32001));
    assert_eq!(body["error"]["message"], json!("Task not found"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_task_owned_by_another_user_reads_as_not_found() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let r = repos(&pool);
    let (owner, owner_session) = seed_user_and_session(&pool).await;
    let (_, task_id) = seed_context_and_task(&r, &owner, &owner_session).await;
    let (stranger, stranger_session) = seed_user_and_session(&pool).await;
    let (stranger_ctx, _) = seed_context_and_task(&r, &stranger, &stranger_session).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&stranger_ctx, &stranger_session, &stranger, "test_agent");

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "GetTask",
        "params": {"id": task_id.as_str()},
        "id": 6
    });
    let response = call(state, Some(ctx), &payload.to_string()).await;
    let (status, body) = body_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["error"]["code"],
        json!(-32001),
        "another user's task is not disclosed: {body}"
    );
    assert!(body.get("result").is_none());

    r.tasks.delete_task(&task_id).await.ok();
}
