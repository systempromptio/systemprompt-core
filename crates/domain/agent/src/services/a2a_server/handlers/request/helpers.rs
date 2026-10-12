//! A2A request handler helpers, including the stream `Retry-After` hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{KeepAlive, Sse};
use std::sync::Arc;
use systemprompt_models::RequestContext;

use super::super::state::AgentHandlerState;
use super::streaming::handle_streaming_request;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{A2aJsonRpcRequest, A2aParseError, A2aRequestParams};
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;

pub async fn parse_a2a_request(
    jsonrpc_request: &A2aJsonRpcRequest,
    request_id: &NumberOrString,
) -> Result<A2aRequestParams, axum::response::Response> {
    jsonrpc_request.parse_request().map_err(|e| {
        let builder = match &e {
            A2aParseError::UnsupportedMethod { method } => JsonRpcErrorBuilder::method_not_found()
                .with_metadata("method", method)
                .log_warn(format!("Unsupported A2A method '{method}'")),
            A2aParseError::InvalidParams { method, source } => {
                let builder = JsonRpcErrorBuilder::invalid_params()
                    .with_metadata("method", method)
                    .with_metadata("detail", source.to_string());
                let builder = if source.to_string().contains("contextId") {
                    builder.with_metadata(
                        "hint",
                        "message.contextId is required: create a context with POST \
                         /api/v1/core/contexts using your JWT",
                    )
                } else {
                    builder
                };
                builder.log_warn(format!("Invalid params for A2A method '{method}': {e}"))
            },
        };
        let (status, error_response) = builder.build_with_status(request_id);
        (status, Json(error_response)).into_response()
    })
}

const STREAM_RETRY_AFTER_SECONDS: u32 = 5;

pub async fn handle_streaming_path(
    a2a_request: A2aRequestParams,
    state: Arc<AgentHandlerState>,
    request_id: NumberOrString,
    context: RequestContext,
    start_time: std::time::Instant,
) -> axum::response::Response {
    tracing::info!("Processing SendStreamingMessage request with SSE response");

    let Ok(stream) =
        handle_streaming_request(a2a_request, state, request_id.clone(), context).await
    else {
        tracing::warn!("Streaming request rejected: global stream-concurrency cap reached");

        let error_response = JsonRpcErrorBuilder::internal_error()
            .with_metadata(
                "detail",
                "Server stream-concurrency limit reached; retry shortly",
            )
            .build(&request_id);

        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(
                http::header::RETRY_AFTER,
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
