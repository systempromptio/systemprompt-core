//! A2A JSON-RPC request dispatch.
//!
//! [`handle_agent_request`] is the single entry point: it parses the JSON-RPC
//! envelope, negotiates the A2A protocol version, enforces OAuth when
//! required, derives the request context, and routes to the streaming or
//! non-streaming handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_run;
pub mod helpers;
pub mod list_tasks;
mod non_streaming;
mod run_stream;
mod streaming;
pub mod validation;

use axum::body::Bytes;
use axum::extract::{Extension, Json, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::a2a::methods;

use super::state::AgentHandlerState;
use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{A2aJsonRpcRequest, A2aRequestParams};
use crate::services::a2a_server::auth::validate_oauth_for_request;
use crate::services::a2a_server::errors::JsonRpcErrorBuilder;
use crate::services::a2a_server::version::negotiate;

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
    uri: Uri,
    body: Bytes,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();

    let Some(Extension(context)) = context else {
        tracing::error!(
            "RequestContext missing from request extensions - middleware configuration error"
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                JsonRpcErrorBuilder::internal_error()
                    .with_metadata("detail", "request context unavailable")
                    .build_unidentified(),
            ),
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

    if let Err(refusal) = negotiate(&headers, &uri) {
        let (status, error_response) = refusal.build_with_status(&request_id);
        return (status, Json(error_response)).into_response();
    }

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

    let response =
        match handle_non_streaming_request(a2a_request, &state, &enriched_context, &request_id)
            .await
        {
            Ok(response) => Json(response).into_response(),
            Err(failure) => Json(failure.into_jsonrpc(&request_id)).into_response(),
        };

    let latency_ms = start_time.elapsed().as_millis();
    let latency_ms = i64::try_from(latency_ms).unwrap_or(i64::MAX);
    tracing::info!(latency_ms = %latency_ms, oauth = %requires_oauth, method = %jsonrpc_request.method, "A2A request processed");

    response
}

fn parse_json_rpc_body(
    body_bytes: &[u8],
) -> Result<A2aJsonRpcRequest, Box<axum::response::Response>> {
    let payload: serde_json::Value = match serde_json::from_slice(body_bytes) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::warn!(topic = "a2a_jsonrpc", error = %e, "Unparseable A2A request body");
            let error_response = JsonRpcErrorBuilder::parse_error()
                .with_metadata("detail", "Invalid JSON")
                .build_unidentified();
            return Err(Box::new(
                (StatusCode::BAD_REQUEST, Json(error_response)).into_response(),
            ));
        },
    };

    let id = payload
        .get("id")
        .and_then(|id| serde_json::from_value::<NumberOrString>(id.clone()).ok());

    match serde_json::from_value::<A2aJsonRpcRequest>(payload) {
        Ok(request) => Ok(request),
        Err(e) => {
            tracing::warn!(topic = "a2a_jsonrpc", error = %e, "Invalid JSON-RPC request");
            let error_response = JsonRpcErrorBuilder::invalid_request().with_metadata(
                "detail",
                "Request must be valid JSON-RPC 2.0 with jsonrpc, method, params, and id",
            );
            let response = match id {
                Some(id) => {
                    (StatusCode::BAD_REQUEST, Json(error_response.build(&id))).into_response()
                },
                None => (
                    StatusCode::BAD_REQUEST,
                    Json(error_response.build_unidentified()),
                )
                    .into_response(),
            };
            Err(Box::new(response))
        },
    }
}

async fn enforce_oauth(
    state: &AgentHandlerState,
    headers: &HeaderMap,
    request_id: &NumberOrString,
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
