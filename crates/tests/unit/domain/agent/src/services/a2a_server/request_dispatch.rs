// Tests for the A2A request-dispatch helpers: routing push-notification
// config requests (set/get/delete answered, list and non-push declined, and
// the request-id injection into the JSON-RPC envelope) and the streaming
// path's concurrency-cap rejection (zero permits yields 503 + Retry-After).

use std::sync::Arc;

use axum::http::StatusCode;
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    ListTaskPushNotificationConfigRequest, MessageSendParams, PushNotificationConfig,
    SetTaskPushNotificationConfigRequest,
};
use systemprompt_agent::models::a2a::{A2aRequestParams, Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::handlers::request::helpers::{
    handle_push_notification_requests, handle_streaming_path,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_helpers::{StubAiProvider, make_handler_state, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn push_config(url: &str) -> PushNotificationConfig {
    PushNotificationConfig {
        endpoint: String::new(),
        headers: None,
        url: url.to_owned(),
        token: None,
        authentication: None,
    }
}

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

#[tokio::test]
async fn push_dispatch_set_get_delete_answer_with_request_id() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (_ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let request_id = NumberOrString::String("req-push-1".to_owned());
    let start = std::time::Instant::now();

    let set =
        A2aRequestParams::SetTaskPushNotificationConfig(SetTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
            config: push_config("https://example.invalid/hook"),
        });
    let response = handle_push_notification_requests(&set, &state, &request_id, start)
        .await
        .expect("set is a push request");
    assert_eq!(response.status(), StatusCode::OK);

    let numeric_id = NumberOrString::Number(7);
    let get =
        A2aRequestParams::GetTaskPushNotificationConfig(GetTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
        });
    let response = handle_push_notification_requests(&get, &state, &numeric_id, start)
        .await
        .expect("get is a push request");
    assert_eq!(response.status(), StatusCode::OK);

    let del = A2aRequestParams::DeleteTaskPushNotificationConfig(
        DeleteTaskPushNotificationConfigRequest {
            task_id: task_id.clone(),
        },
    );
    let response = handle_push_notification_requests(&del, &state, &request_id, start)
        .await
        .expect("delete is a push request");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn push_dispatch_declines_list_and_non_push_requests() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let state = make_handler_state(&pool, Arc::new(StubAiProvider::new()), 4);
    let request_id = NumberOrString::Number(1);
    let start = std::time::Instant::now();

    let list =
        A2aRequestParams::ListTaskPushNotificationConfig(ListTaskPushNotificationConfigRequest {
            task_id: TaskId::generate(),
            limit: None,
            offset: None,
        });
    assert!(
        handle_push_notification_requests(&list, &state, &request_id, start)
            .await
            .is_none()
    );

    let ctx = ContextId::generate();
    let send = A2aRequestParams::SendMessage(send_params(&ctx));
    assert!(
        handle_push_notification_requests(&send, &state, &request_id, start)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn streaming_path_with_no_permits_returns_service_unavailable() {
    let Some(pool) = try_pool().await else {
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
    let Some(pool) = try_pool().await else {
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
}
