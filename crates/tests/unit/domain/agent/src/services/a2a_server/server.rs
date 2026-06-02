// Tests for the A2A server state and SSE-stream entry point. `create_sse_stream`
// is driven both when a permit is available (stream returned, the spawned task
// then fails to load the unconfigured agent and exits quietly) and when the
// global concurrency cap is exhausted (StreamRejected). The handler-state
// Debug/Clone surface is exercised directly.

use std::sync::Arc;

use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::streaming::{
    create_sse_stream, CreateSseStreamParams, StreamRejected,
};
use systemprompt_identifiers::{ContextId, MessageId, SessionId, UserId};

use super::a2a_helpers::{make_handler_state, request_context, StubAiProvider};
use crate::repository::try_pool;

fn message(ctx: &ContextId) -> Message {
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
async fn create_sse_stream_returns_stream_when_permit_available() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 4);

    let ctx = ContextId::generate();
    let session = SessionId::generate();
    let user = UserId::new("u-sse");
    let request = request_context(&ctx, &session, &user, "no_such_agent");

    let result = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx),
        agent_name: "no_such_agent".to_owned(),
        state,
        request_id: RequestId::Number(1),
        context: request,
        callback_config: None,
    })
    .await;

    assert!(
        result.is_ok(),
        "a permit was available, so a stream must be returned"
    );
}

#[tokio::test]
async fn create_sse_stream_rejected_when_cap_exhausted() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    // Zero permits: every stream request is rejected.
    let state = make_handler_state(&pool, provider, 0);

    let ctx = ContextId::generate();
    let session = SessionId::generate();
    let user = UserId::new("u-sse");
    let request = request_context(&ctx, &session, &user, "no_such_agent");

    let result = create_sse_stream(CreateSseStreamParams {
        message: message(&ctx),
        agent_name: "no_such_agent".to_owned(),
        state,
        request_id: RequestId::Number(2),
        context: request,
        callback_config: None,
    })
    .await;

    assert!(matches!(result, Err(StreamRejected)));
}

#[tokio::test]
async fn handler_state_debug_and_clone() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let provider = Arc::new(StubAiProvider::new());
    let state = make_handler_state(&pool, provider, 2);

    let debug = format!("{:?}", state);
    assert!(debug.contains("AgentHandlerState"));

    let cloned = Arc::clone(&state);
    assert_eq!(
        cloned.stream_semaphore.available_permits(),
        state.stream_semaphore.available_permits()
    );
}
