//! A2A JSON-RPC request dispatch.
//!
//! [`handle_agent_request`] is the single entry point: it parses the JSON-RPC
//! envelope, enforces OAuth when required, derives the request context, and
//! routes to the streaming, push-notification, or non-streaming handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod helpers;
mod non_streaming;
mod streaming;
pub mod validation;

use axum::body::Bytes;
use axum::extract::{Extension, Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;

use super::state::AgentHandlerState;
use crate::models::a2a::A2aRequestParams;
use crate::services::a2a_server::auth::validate_oauth_for_request;
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;

use helpers::{handle_streaming_path, parse_a2a_request};
use non_streaming::handle_non_streaming_request;
use validation::should_require_oauth;

// Why: the body is read through the `Bytes` extractor so the router's
// `DefaultBodyLimit` applies and an oversized A2A payload is refused with 413
// before it is buffered.
pub async fn handle_agent_request(
    State(state): State<Arc<AgentHandlerState>>,
    context: Option<Extension<RequestContext>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();

    let Some(Extension(context)) = context else {
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
    };

    tracing::info!("Agent request handler invoked");

    let jsonrpc_request = match parse_json_rpc_body(&body) {
        Ok(req) => req,
        Err(response) => return *response,
    };

    let request_id = jsonrpc_request.id.clone();
    tracing::info!(method = %jsonrpc_request.method, "Processing A2A JSON-RPC method");

    let requires_oauth = should_require_oauth(&state).await;

    if requires_oauth && let Err(response) = enforce_oauth(&state, &headers, &request_id).await {
        return response;
    }

    let is_streaming = jsonrpc_request.method == methods::SEND_STREAMING_MESSAGE;

    let a2a_request = match parse_a2a_request(&jsonrpc_request, &request_id).await {
        Ok(req) => req,
        Err(response) => return response,
    };

    let mut enriched_context = context;
    match &a2a_request {
        A2aRequestParams::SendMessage(params) | A2aRequestParams::SendStreamingMessage(params) => {
            enriched_context = enriched_context.with_context_id(params.message.context_id.clone());
        },
        _ => {},
    }

    if is_streaming {
        return handle_streaming_path(a2a_request, state, request_id, enriched_context, start_time)
            .await;
    }

    let response_result =
        handle_non_streaming_request(a2a_request, &state, &enriched_context).await;

    let json_rpc_response = build_json_rpc_response(response_result, &request_id);

    let latency_ms = start_time.elapsed().as_millis();
    let latency_ms = i64::try_from(latency_ms).unwrap_or(i64::MAX);
    tracing::info!(latency_ms = %latency_ms, oauth = %requires_oauth, method = %jsonrpc_request.method, "A2A request processed");

    (StatusCode::OK, Json(json_rpc_response)).into_response()
}

fn parse_json_rpc_body(
    body_bytes: &[u8],
) -> Result<crate::models::a2a::A2aJsonRpcRequest, Box<axum::response::Response>> {
    let payload: serde_json::Value = match serde_json::from_slice(body_bytes) {
        Ok(p) => p,
        Err(_) => {
            return Err(Box::new(
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32700, "message": "Invalid JSON"},
                        "id": null
                    })),
                )
                    .into_response(),
            ));
        },
    };

    serde_json::from_value::<crate::models::a2a::A2aJsonRpcRequest>(payload).map_err(|e| {
        let error_response = JsonRpcErrorBuilder::invalid_request()
            .with_data(json!(
                "Request must be valid JSON-RPC 2.0 with jsonrpc, method, params, and id"
            ))
            .log_error(format!("Invalid JSON-RPC request: {e}"))
            .build(&crate::models::a2a::jsonrpc::NumberOrString::Number(0));
        Box::new((StatusCode::BAD_REQUEST, Json(error_response)).into_response())
    })
}

async fn enforce_oauth(
    state: &AgentHandlerState,
    headers: &HeaderMap,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
) -> Result<(), axum::response::Response> {
    tracing::info!("Request requires OAuth2 authentication");

    let required_scopes = {
        let config = state.config.read().await;
        config.oauth.scopes.clone()
    };

    validate_oauth_for_request(
        headers,
        request_id,
        &required_scopes,
        state.oauth_state.jwt_provider.as_ref(),
    )
    .await
    .map(|_| ())
    .map_err(|(status, error_response)| (status, Json(error_response)).into_response())
}

fn build_json_rpc_response(
    response_result: Result<crate::models::a2a::Task, non_streaming::RequestFailure>,
    request_id: &crate::models::a2a::jsonrpc::NumberOrString,
) -> serde_json::Value {
    match response_result {
        Ok(task) => match serde_json::to_value(task) {
            Ok(task_value) => json!({
                "jsonrpc": "2.0",
                "result": task_value,
                "id": request_id
            }),
            Err(e) => JsonRpcErrorBuilder::internal_error()
                .with_data(json!("Task serialization failed"))
                .log_error(format!("Failed to serialize task response: {e}"))
                .build(request_id),
        },
        Err(failure) => failure.into_jsonrpc(request_id),
    }
}
