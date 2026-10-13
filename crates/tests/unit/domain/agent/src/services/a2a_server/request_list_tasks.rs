// ListTasks through handle_agent_request: caller-bound listing, filters,
// paging with the opaque page token, and the invalid-token envelope.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_agent::services::a2a_server::handlers::request::handle_agent_request;
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::{PROTOCOL_VERSION, VERSION_HEADER};

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;

async fn list(state: Arc<AgentHandlerState>, context: RequestContext, params: Value) -> Value {
    let mut headers = HeaderMap::new();
    headers.insert(VERSION_HEADER, HeaderValue::from_static(PROTOCOL_VERSION));
    let body = json!({"jsonrpc": "2.0", "method": "ListTasks", "params": params, "id": 21});
    let response = handle_agent_request(
        State(state),
        Some(Extension(context)),
        headers,
        Uri::from_static("/api/v1/agents/test_agent"),
        Bytes::from(body.to_string()),
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json body")
}

fn task_ids(body: &Value) -> Vec<String> {
    body["result"]["tasks"]
        .as_array()
        .unwrap_or_else(|| panic!("tasks array: {body}"))
        .iter()
        .map(|t| t["id"].as_str().expect("task id").to_owned())
        .collect()
}

#[tokio::test]
async fn list_tasks_returns_only_the_callers_tasks_paged() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let mut owned = Vec::new();
    for _ in 0..3 {
        owned.push(seed_context_and_task(&r, &owner, &session).await);
    }
    let (stranger, stranger_session) = seed_user_and_session(&pool).await;
    let (_, foreign_task) = seed_context_and_task(&r, &stranger, &stranger_session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&owned[0].0, &session, &owner, "test_agent");

    let first = list(Arc::clone(&state), ctx.clone(), json!({"pageSize": 2})).await;
    assert_eq!(first["id"], 21);
    assert!(first["result"].get("totalSize").is_none(), "{first}");
    assert_eq!(first["result"]["pageSize"], 2);
    let token = first["result"]["nextPageToken"].as_str().expect("token");
    assert!(!token.is_empty(), "a further page exists: {first}");

    let second = list(
        Arc::clone(&state),
        ctx.clone(),
        json!({"pageSize": 2, "pageToken": token}),
    )
    .await;
    assert_eq!(second["result"]["nextPageToken"], "", "{second}");

    let mut seen = task_ids(&first);
    seen.extend(task_ids(&second));
    seen.sort();
    let mut expected: Vec<String> = owned.iter().map(|(_, t)| t.as_str().to_owned()).collect();
    expected.sort();
    assert_eq!(seen, expected);
    assert!(!seen.contains(&foreign_task.as_str().to_owned()));

    for (_, task_id) in &owned {
        r.tasks.delete_task(task_id).await.ok();
    }
    r.tasks.delete_task(&foreign_task).await.ok();
}

#[tokio::test]
async fn list_tasks_filters_by_context_and_status() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (ctx_a, task_a) = seed_context_and_task(&r, &owner, &session).await;
    let (_, task_b) = seed_context_and_task(&r, &owner, &session).await;
    r.tasks
        .update_task_state(&task_b, TaskState::Working, &chrono::Utc::now())
        .await
        .expect("move task to working");

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&ctx_a, &session, &owner, "test_agent");

    let by_context = list(
        Arc::clone(&state),
        ctx.clone(),
        json!({"contextId": ctx_a.as_str()}),
    )
    .await;
    assert_eq!(task_ids(&by_context), [task_a.as_str()]);
    assert_eq!(by_context["result"]["nextPageToken"], "");

    let by_status = list(
        Arc::clone(&state),
        ctx.clone(),
        json!({"status": "TASK_STATE_WORKING"}),
    )
    .await;
    assert_eq!(task_ids(&by_status), [task_b.as_str()]);

    let future = list(
        state,
        ctx,
        json!({"statusTimestampAfter": "2999-01-01T00:00:00Z"}),
    )
    .await;
    assert!(task_ids(&future).is_empty(), "{future}");
    assert_eq!(future["result"]["nextPageToken"], "");

    r.tasks.delete_task(&task_a).await.ok();
    r.tasks.delete_task(&task_b).await.ok();
}

#[tokio::test]
async fn list_tasks_omits_artifacts_unless_asked() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (ctx_a, task_a) = seed_context_and_task(&r, &owner, &session).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&ctx_a, &session, &owner, "test_agent");

    let body = list(state, ctx, json!({"historyLength": 0})).await;
    let task = &body["result"]["tasks"][0];
    assert!(task.get("artifacts").is_none(), "{body}");
    assert!(task.get("history").is_none(), "{body}");

    r.tasks.delete_task(&task_a).await.ok();
}

#[tokio::test]
async fn an_invalid_page_token_is_an_invalid_params_error() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (ctx_a, task_a) = seed_context_and_task(&r, &owner, &session).await;
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let ctx = request_context(&ctx_a, &session, &owner, "test_agent");

    let body = list(state, ctx, json!({"pageToken": "%%%"})).await;
    assert_eq!(body["error"]["code"], -32602, "{body}");
    assert_eq!(body["error"]["data"][0]["reason"], "INVALID_PARAMS");
    assert!(body.get("result").is_none());

    r.tasks.delete_task(&task_a).await.ok();
}
