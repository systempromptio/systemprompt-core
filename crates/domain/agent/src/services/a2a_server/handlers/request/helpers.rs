use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::sse::{KeepAlive, Sse};
use axum::response::IntoResponse;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;

use super::super::state::AgentHandlerState;
use super::streaming::handle_streaming_request;
use crate::models::a2a::A2aRequestParams;
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;

pub async fn parse_a2a_request(
    jsonrpc_request: &crate::models::a2a::A2aJsonRpcRequest,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
) -> Result<A2aRequestParams, axum::response::Response> {
    match jsonrpc_request.parse_request() {
        Ok(request) => Ok(request),
        Err(e) => {
            let error_str = e.to_string();

            if error_str.contains("missing field `contextId`") {
                let helpful_message = json!({
                    "error": "contextId is required",
                    "message": "JWT token and contextId are required to use this API.",
                    "instructions": {
                        "step1": {
                            "description": "Obtain a JWT token (no registration required)",
                            "endpoint": "POST /api/v1/core/oauth/session"
                        },
                        "step2": {
                            "description": "Create a context using your JWT token",
                            "endpoint": "POST /api/v1/core/contexts"
                        },
                        "step3": {
                            "description": "Include contextId in your message/stream request"
                        }
                    }
                });

                let error_response = JsonRpcErrorBuilder::invalid_params()
                    .with_data(helpful_message)
                    .log_error("Missing required contextId in message/stream request".to_string())
                    .build(request_id)
                    .await;
                Err((StatusCode::BAD_REQUEST, Json(error_response)).into_response())
            } else {
                let error_response = JsonRpcErrorBuilder::method_not_found()
                    .with_data(json!(format!(
                        "Unsupported method: {}",
                        jsonrpc_request.method
                    )))
                    .log_error(format!(
                        "Invalid A2A request method '{}': {}",
                        jsonrpc_request.method, e
                    ))
                    .build(request_id)
                    .await;
                Err((StatusCode::BAD_REQUEST, Json(error_response)).into_response())
            }
        },
    }
}

pub async fn handle_streaming_path(
    a2a_request: A2aRequestParams,
    state: Arc<AgentHandlerState>,
    request_id: crate::models::a2a::jsonrpc::NumberOrString,
    context: RequestContext,
    start_time: std::time::Instant,
) -> axum::response::Response {
    tracing::info!("Processing message/stream request with SSE response");

    let stream = handle_streaming_request(a2a_request, state, request_id, context).await;

    let latency_ms = start_time.elapsed().as_millis();
    tracing::info!(latency_ms = %latency_ms, "SSE stream initialized for message/stream");

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub async fn handle_push_notification_requests(
    a2a_request: &A2aRequestParams,
    state: &AgentHandlerState,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
    start_time: std::time::Instant,
) -> Option<axum::response::Response> {
    let push_notification_response = match a2a_request {
        A2aRequestParams::SetTaskPushNotificationConfig(params) => {
            use crate::services::a2a_server::handlers::push_notification_config::handle_set_push_notification_config;

            tracing::info!("Handling tasks/pushNotificationConfig/set request");

            Some(
                handle_set_push_notification_config(State(Arc::new(state.clone())), params.clone())
                    .await,
            )
        },
        A2aRequestParams::GetTaskPushNotificationConfig(params) => {
            use crate::services::a2a_server::handlers::push_notification_config::handle_get_push_notification_config;

            tracing::info!("Handling tasks/pushNotificationConfig/get request");

            Some(
                handle_get_push_notification_config(State(Arc::new(state.clone())), params.clone())
                    .await,
            )
        },
        A2aRequestParams::ListTaskPushNotificationConfig(_params) => {
            tracing::info!("Handling tasks/pushNotificationConfig/list request");
            None
        },
        A2aRequestParams::DeleteTaskPushNotificationConfig(params) => {
            use crate::services::a2a_server::handlers::push_notification_config::handle_delete_push_notification_config;

            tracing::info!("Handling tasks/pushNotificationConfig/delete request");

            Some(
                handle_delete_push_notification_config(
                    State(Arc::new(state.clone())),
                    params.clone(),
                )
                .await,
            )
        },
        _ => None,
    };

    if let Some(result) = push_notification_response {
        let (status, json_response) = match result {
            Ok((status, json)) => (status, json),
            Err((status, json)) => (status, json),
        };

        let mut response_value = json_response.0;
        if let Some(obj) = response_value.as_object_mut() {
            obj.insert(
                "id".to_string(),
                match request_id {
                    crate::models::a2a::jsonrpc::NumberOrString::String(s) => {
                        serde_json::Value::String(s.clone())
                    },
                    crate::models::a2a::jsonrpc::NumberOrString::Number(n) => {
                        serde_json::Value::Number(serde_json::Number::from(*n))
                    },
                },
            );
        }

        let latency_ms = start_time.elapsed().as_millis();
        tracing::info!(latency_ms = %latency_ms, "Push notification config request processed");

        return Some((status, Json(response_value)).into_response());
    }

    None
}
