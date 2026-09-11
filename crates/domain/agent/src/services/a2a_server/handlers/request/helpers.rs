//! A2A request handler helpers, including the stream `Retry-After` hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{KeepAlive, Sse};
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
                            "description": "Include contextId in your SendStreamingMessage request"
                        }
                    }
                });

                let error_response = JsonRpcErrorBuilder::invalid_params()
                    .with_data(helpful_message)
                    .log_error(
                        "Missing required contextId in SendStreamingMessage request".to_owned(),
                    )
                    .build(request_id);
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
                    .build(request_id);
                Err((StatusCode::BAD_REQUEST, Json(error_response)).into_response())
            }
        },
    }
}

const STREAM_RETRY_AFTER_SECONDS: u32 = 5;

pub async fn handle_streaming_path(
    a2a_request: A2aRequestParams,
    state: Arc<AgentHandlerState>,
    request_id: crate::models::a2a::jsonrpc::NumberOrString,
    context: RequestContext,
    start_time: std::time::Instant,
) -> axum::response::Response {
    tracing::info!("Processing SendStreamingMessage request with SSE response");

    let Ok(stream) =
        handle_streaming_request(a2a_request, state, request_id.clone(), context).await
    else {
        tracing::warn!("Streaming request rejected: global stream-concurrency cap reached");

        let error_response = JsonRpcErrorBuilder::internal_error()
            .with_data(json!(
                "Server stream-concurrency limit reached; retry shortly"
            ))
            .build(&request_id);

        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(
                axum::http::header::RETRY_AFTER,
                STREAM_RETRY_AFTER_SECONDS.to_string(),
            )],
            Json(error_response),
        )
            .into_response();
    };

    let latency_ms = start_time.elapsed().as_millis();
    tracing::info!(latency_ms = %latency_ms, "SSE stream initialized for SendStreamingMessage");

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
