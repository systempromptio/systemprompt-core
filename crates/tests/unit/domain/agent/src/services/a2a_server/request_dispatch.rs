// Tests for the A2A streaming dispatch path: the concurrency-cap rejection
// (zero permits yields 503 + Retry-After) and the validation-error stream for
// an unknown context.

use std::sync::Arc;

use axum::http::StatusCode;
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::models::a2a::protocol::{MessageSendParams, TaskQueryParams};
use systemprompt_agent::models::a2a::{A2aRequestParams, Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::handlers::request::helpers::handle_streaming_path;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

fn send_params(ctx: &ContextId) -> MessageSendParams {
    MessageSendParams {
        message: Message {
            role: MessageRole::User,
            parts: vec![Part::Text(TextPart {
                text: "hello".to_owned(),
            })],
            message_id: MessageId::generate(),
            context_id: ctx.clone(),
            task_id: None,
            reference_task_ids: None,
            metadata: None,
            extensions: None,
        },
        configuration: None,
        metadata: None,
    }
}

async fn sse_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("SSE body");
    let body = std::str::from_utf8(&bytes).expect("SSE is UTF-8");
    let data = body
        .lines()
        .find_map(|line| line.strip_prefix("data:"))
        .expect("SSE data frame")
        .trim();
    serde_json::from_str(data).expect("SSE data is JSON")
}

#[tokio::test]
async fn streaming_path_with_no_permits_returns_service_unavailable() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _task_id) = seed_context_and_task(&repos, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 0);
    let context = request_context(&ctx, &session, &user, "test_agent");

    let response = handle_streaming_path(
        A2aRequestParams::SendStreamingMessage(send_params(&ctx)),
        Arc::clone(&state),
        NumberOrString::Number(9),
        context,
        std::time::Instant::now(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        response
            .headers()
            .contains_key(axum::http::header::RETRY_AFTER)
    );
}

#[tokio::test]
async fn streaming_path_unknown_context_streams_validation_error() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let (user, session) = seed_user_and_session(&pool).await;
    let ctx = ContextId::generate();

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let context = request_context(&ctx, &session, &user, "test_agent");

    let response = handle_streaming_path(
        A2aRequestParams::SendStreamingMessage(send_params(&ctx)),
        Arc::clone(&state),
        NumberOrString::Number(10),
        context,
        std::time::Instant::now(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let event = sse_json(response).await;
    assert_eq!(event["jsonrpc"], serde_json::json!("2.0"));
    assert_eq!(event["id"], serde_json::json!(10));
    assert_eq!(event["error"]["code"], serde_json::json!(-32602));
    assert!(
        event["error"]["data"]
            .as_str()
            .is_some_and(|data| data.contains("Context validation failed")),
        "the wire error must preserve the validation diagnosis: {event}"
    );
}

#[tokio::test]
async fn streaming_path_rejects_a_non_streaming_method_on_the_wire() {
    let pool = try_pool_or_skip()
        .await
        .expect("agent request dispatch fixture database");
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 1);
    let context = request_context(&ctx, &session, &user, "test_agent");
    let response = handle_streaming_path(
        A2aRequestParams::GetTask(TaskQueryParams {
            id: TaskId::new(task_id.as_str()),
            history_length: None,
        }),
        state,
        NumberOrString::String("wrong-method".to_owned()),
        context,
        std::time::Instant::now(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let event = sse_json(response).await;
    assert_eq!(event["jsonrpc"], serde_json::json!("2.0"));
    assert_eq!(event["id"], serde_json::json!("wrong-method"));
    assert_eq!(event["error"]["code"], serde_json::json!(-32601));
    assert_eq!(
        event["error"]["message"],
        serde_json::json!("Method not found")
    );
    assert_eq!(
        event["error"]["data"],
        serde_json::json!("Only SendStreamingMessage requests are supported for streaming")
    );
}
