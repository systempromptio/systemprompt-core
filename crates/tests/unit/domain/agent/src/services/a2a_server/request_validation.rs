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
    should_require_oauth, validate_message_context,
};
use systemprompt_identifiers::{ContextId, MessageId, UserId};

use super::a2a_helpers::{StubAiProvider, make_handler_state};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

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
    let request = rpc("message/send", {
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
async fn validate_message_context_requires_user_id() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = ContextId::generate();
    let err = validate_message_context(&user_message(&ctx), None, &pool)
        .await
        .expect_err("missing user must be rejected");
    assert!(err.contains("authentication required"), "got: {err}");
}

#[tokio::test]
async fn validate_message_context_rejects_placeholder_user_id() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = ContextId::generate();
    let placeholder = UserId::new("missing-user-id");
    let err = validate_message_context(&user_message(&ctx), Some(&placeholder), &pool)
        .await
        .expect_err("placeholder user must be rejected");
    assert!(err.contains("Authentication required"), "got: {err}");
}

#[tokio::test]
async fn validate_message_context_rejects_foreign_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let stranger = UserId::new("u-stranger");
    let ctx = ContextId::generate();
    let err = validate_message_context(&user_message(&ctx), Some(&stranger), &pool)
        .await
        .expect_err("unowned context must be rejected");
    assert!(err.contains("Context validation failed"), "got: {err}");
}

#[tokio::test]
async fn validate_message_context_accepts_owned_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;

    validate_message_context(&user_message(&ctx), Some(&user), &pool)
        .await
        .expect("owned context must validate");
}

#[tokio::test]
async fn should_require_oauth_reflects_handler_config() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let required = should_require_oauth(&state).await;
    assert!(!required, "test agent config does not require oauth");
}
