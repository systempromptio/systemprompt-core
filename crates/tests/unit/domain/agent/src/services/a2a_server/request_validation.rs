// Tests for the A2A request-dispatch helpers: JSON-RPC envelope parsing
// (`parse_a2a_request` error classification) and message-context validation
// (`validate_message_context` auth and ownership checks).

use std::sync::Arc;

use axum::http::StatusCode;
use systemprompt_agent::models::a2a::A2aJsonRpcRequest;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::A2aRequestParams;
use systemprompt_agent::services::a2a_server::handlers::request::helpers::parse_a2a_request;
use systemprompt_agent::services::a2a_server::handlers::request::validation::{
    ContextValidationError, should_require_oauth, validate_message_context, validate_task_owner,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId, UserId};

use super::a2a_helpers::{StubAiProvider, make_handler_state};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

fn rpc(method: &str, params: serde_json::Value) -> A2aJsonRpcRequest {
    A2aJsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        method: method.to_owned(),
        params,
        id: RequestId::Number(7),
    }
}

fn a2a_message_value(ctx: &ContextId) -> serde_json::Value {
    serde_json::json!({
        "message": serde_json::to_value(user_message(ctx)).expect("serialize message")
    })
}

#[tokio::test]
async fn parse_a2a_request_accepts_valid_send_message() {
    let ctx = ContextId::generate();
    let request = rpc(
        systemprompt_models::a2a::methods::SEND_MESSAGE,
        a2a_message_value(&ctx),
    );
    let id = RequestId::Number(7);

    let parsed = parse_a2a_request(&request, &id)
        .await
        .map_err(|_| ())
        .expect("valid message/send must parse");
    match parsed {
        A2aRequestParams::SendMessage(params) => {
            assert_eq!(params.message.context_id, ctx);
        },
        other => panic!("expected SendMessage variant, got {other:?}"),
    }
}

#[tokio::test]
async fn parse_a2a_request_missing_context_id_is_bad_request() {
    let request = rpc(systemprompt_models::a2a::methods::SEND_STREAMING_MESSAGE, {
        let mut value =
            serde_json::to_value(user_message(&ContextId::generate())).expect("serialize");
        value.as_object_mut().expect("object").remove("contextId");
        serde_json::json!({"message": value})
    });
    let id = RequestId::Number(7);

    let response = parse_a2a_request(&request, &id)
        .await
        .expect_err("missing contextId must be rejected");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("read JSON-RPC error body");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("valid JSON-RPC error response");
    assert_eq!(
        payload
            .pointer("/error/data/error")
            .and_then(serde_json::Value::as_str),
        Some("contextId is required"),
        "unexpected JSON-RPC remediation envelope: {payload}"
    );
    assert_eq!(
        payload
            .pointer("/error/data/instructions/step1/endpoint")
            .and_then(serde_json::Value::as_str),
        Some("POST /api/v1/core/oauth/session"),
        "unexpected JWT acquisition instruction: {payload}"
    );
    assert_eq!(
        payload
            .pointer("/error/data/instructions/step2/endpoint")
            .and_then(serde_json::Value::as_str),
        Some("POST /api/v1/core/contexts"),
        "unexpected context creation instruction: {payload}"
    );
}

#[tokio::test]
async fn parse_a2a_request_unknown_method_is_bad_request() {
    let request = rpc("no/such/method", serde_json::json!({}));
    let id = RequestId::Number(7);

    let response = parse_a2a_request(&request, &id)
        .await
        .expect_err("unknown method must be rejected");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

fn user_message(ctx: &ContextId) -> systemprompt_agent::models::a2a::Message {
    use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "hi".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[tokio::test]
async fn validate_message_context_rejects_an_empty_user_id() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let ctx = ContextId::generate();
    let anonymous = UserId::new("");
    let err = validate_message_context(&user_message(&ctx), &anonymous, &repos(&pool).contexts)
        .await
        .expect_err("an empty identity must be rejected");
    assert!(
        matches!(err, ContextValidationError::Unauthenticated),
        "got: {err}"
    );
}

#[tokio::test]
async fn validate_message_context_rejects_foreign_context() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let stranger = UserId::new("u-stranger");
    let ctx = ContextId::generate();
    let err = validate_message_context(&user_message(&ctx), &stranger, &repos(&pool).contexts)
        .await
        .expect_err("unowned context must be rejected");
    assert!(
        matches!(err, ContextValidationError::Context(_)),
        "got: {err}"
    );
    assert!(
        err.to_string().contains("Context validation failed"),
        "got: {err}"
    );
}

#[tokio::test]
async fn validate_task_owner_answers_not_found_for_another_users_task() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let repos = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (_, task_id) = seed_context_and_task(&repos, &owner, &session).await;

    validate_task_owner(&repos.tasks, &task_id, &owner)
        .await
        .expect("the owner passes");

    let stranger = UserId::new("u-stranger");
    let err = validate_task_owner(&repos.tasks, &task_id, &stranger)
        .await
        .expect_err("another user must not see the task");
    assert!(
        matches!(err, ContextValidationError::TaskNotFound(ref id) if *id == task_id),
        "a foreign task reads as absent, never as forbidden: {err}"
    );

    let err = validate_task_owner(&repos.tasks, &TaskId::generate(), &owner)
        .await
        .expect_err("an unknown task is not found");
    assert!(
        matches!(err, ContextValidationError::TaskNotFound(_)),
        "{err}"
    );
}

#[tokio::test]
async fn validate_message_context_accepts_owned_context() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;

    validate_message_context(&user_message(&ctx), &user, &repos.contexts)
        .await
        .expect("owned context must validate");
}

#[tokio::test]
async fn should_require_oauth_reflects_handler_config() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let required = should_require_oauth(&state).await;
    assert!(!required, "test agent config does not require oauth");
}
