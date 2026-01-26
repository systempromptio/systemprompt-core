mod helpers;
mod non_streaming;
mod streaming;
mod validation;

use axum::extract::{Json, Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;

use super::state::AgentHandlerState;
use crate::models::a2a::A2aRequestParams;
use crate::services::a2a_server::auth::validate_oauth_for_request;
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;

use helpers::{handle_push_notification_requests, handle_streaming_path, parse_a2a_request};
use non_streaming::handle_non_streaming_request;
use validation::should_require_oauth;

pub async fn handle_agent_request(
    State(state): State<Arc<AgentHandlerState>>,
    request: Request,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();

    let context = match request.extensions().get::<RequestContext>().cloned() {
        Some(ctx) => ctx,
        None => {
            tracing::error!(
                "RequestContext missing from request extensions - middleware configuration error"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32603, "message": "Internal server error: request context unavailable"},
                    "id": null
                })),
            )
                .into_response();
        },
    };

    tracing::info!("Agent request handler invoked");

    let (parts, body) = request.into_parts();
    let headers = parts.headers.clone();

    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32700, "message": "Failed to read request body"},
                    "id": null
                })),
            )
                .into_response();
        },
    };

    let payload: serde_json::Value = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32700, "message": "Invalid JSON"},
                    "id": null
                })),
            )
                .into_response();
        },
    };

    let jsonrpc_request =
        match serde_json::from_value::<crate::models::a2a::A2aJsonRpcRequest>(payload) {
            Ok(req) => req,
            Err(e) => {
                let error_response = JsonRpcErrorBuilder::invalid_request()
                    .with_data(json!(
                        "Request must be valid JSON-RPC 2.0 with jsonrpc, method, params, and id"
                    ))
                    .log_error(format!("Invalid JSON-RPC request: {e}"))
                    .build(&crate::models::a2a::jsonrpc::NumberOrString::Number(0))
                    .await;
                return (StatusCode::BAD_REQUEST, Json(error_response)).into_response();
            },
        };

    let request_id = jsonrpc_request.id.clone();
    tracing::info!(method = %jsonrpc_request.method, "Processing A2A JSON-RPC method");

    let requires_oauth = should_require_oauth(&jsonrpc_request, &state).await;

    if requires_oauth {
        tracing::info!("Request requires OAuth2 authentication");

        let required_scopes = {
            let config = state.config.read().await;
            config.oauth.scopes.clone()
        };

        if let Err((status, error_response)) = validate_oauth_for_request(
            &headers,
            &request_id,
            &required_scopes,
            state.oauth_state.jwt_provider.as_ref(),
        )
        .await
        {
            return (status, Json(error_response)).into_response();
        }
    }

    let is_streaming = jsonrpc_request.method == "message/stream";

    let a2a_request = match parse_a2a_request(&jsonrpc_request, &request_id).await {
        Ok(req) => req,
        Err(response) => return response,
    };

    let mut enriched_context = context.clone();
    match &a2a_request {
        A2aRequestParams::SendMessage(ref params)
        | A2aRequestParams::SendStreamingMessage(ref params) => {
            if params.message.context_id.as_str().is_empty() {
                let error_response = JsonRpcErrorBuilder::invalid_params()
                    .with_data(json!({
                        "error": "contextId cannot be empty",
                        "message": "contextId must be a valid non-empty string. Please create a context first using POST /api/v1/core/contexts"
                    }))
                    .log_error("Rejected request with empty contextId".to_string())
                    .build(&request_id)
                    .await;
                return (StatusCode::BAD_REQUEST, Json(error_response)).into_response();
            }
            enriched_context = enriched_context.with_context_id(params.message.context_id.clone());
        },
        _ => {},
    }

    if is_streaming {
        return handle_streaming_path(a2a_request, state, request_id, enriched_context, start_time)
            .await;
    }

    if let Some(response) =
        handle_push_notification_requests(&a2a_request, &state, &request_id, start_time).await
    {
        return response;
    }

    let response_result =
        handle_non_streaming_request(a2a_request, &state, &enriched_context).await;

    let json_rpc_response = match response_result {
        Ok(task) => match serde_json::to_value(task) {
            Ok(task_value) => json!({
                "jsonrpc": "2.0",
                "result": task_value,
                "id": request_id
            }),
            Err(e) => {
                JsonRpcErrorBuilder::internal_error()
                    .with_data(json!("Task serialization failed"))
                    .log_error(format!("Failed to serialize task response: {e}"))
                    .build(&request_id)
                    .await
            },
        },
        Err(e) => {
            JsonRpcErrorBuilder::internal_error()
                .with_data(json!(format!("Request handling failed: {e}")))
                .log_error(format!("A2A request handling failed: {e}"))
                .build(&request_id)
                .await
        },
    };

    let latency_ms = start_time.elapsed().as_millis();
    let latency_ms = i64::try_from(latency_ms).unwrap_or(i64::MAX);
    tracing::info!(latency_ms = %latency_ms, oauth = %requires_oauth, method = %jsonrpc_request.method, "A2A request processed");

    (StatusCode::OK, Json(json_rpc_response)).into_response()
}
