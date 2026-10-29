//! `POST /v1/messages/count_tokens`: a token count for a Messages body.
//!
//! Claude Code asks for counts while it sizes its context, so the route must
//! answer rather than 404. It authenticates exactly like `/messages` (bridge
//! JWT, OAuth JWT or API key through [`messages::auth::authenticate`]), but it
//! is not inference: no route is dispatched, no governance chain, quota or
//! safety scan runs, and no `ai_requests` row is written.
//!
//! The count is an estimate on every provider: one token per four characters
//! of the system prompt, message content and tool definitions, rounded up.
//! Forwarding to an upstream counter would need the full provider credential
//! and endpoint resolution of the dispatch path for a figure the client only
//! uses to decide when to compact, so the estimate is the contract. Only a
//! missing or invalid credential is refused; a body that cannot be read
//! counts as zero rather than failing the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_runtime::AppContext;

use super::messages::auth::{AuthInputs, authenticate};
use super::messages::error::RejectionError;
use super::messages::{GatewayHandles, extract_credential};
use crate::services::middleware::JwtContextExtractor;

const CHARS_PER_TOKEN: usize = 4;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    handles: GatewayHandles,
    request: Request<Body>,
) -> Response<Body> {
    match count(&jwt_extractor, &ctx, &handles, request).await {
        Ok(input_tokens) => json_response(
            StatusCode::OK,
            serde_json::json!({ "input_tokens": input_tokens }).to_string(),
        ),
        Err(rejection) => {
            tracing::warn!(
                status = %rejection.status,
                message = %rejection.message,
                cause = ?rejection.cause,
                "Gateway count_tokens rejected",
            );
            let body = AnthropicMessagesInbound
                .render_error(rejection.status, rejection.public_message());
            json_response(rejection.status, body)
        },
    }
}

async fn count(
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
    handles: &GatewayHandles,
    request: Request<Body>,
) -> Result<usize, RejectionError> {
    let enabled = ctx
        .services_config()
        .gateway_config()
        .is_some_and(|g| g.enabled);
    if !enabled {
        return Err(RejectionError::client(
            StatusCode::NOT_FOUND,
            "Gateway not enabled",
        ));
    }
    let credential = extract_credential(request.headers()).ok_or_else(|| {
        RejectionError::client(
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential",
        )
    })?;
    authenticate(
        &credential,
        AuthInputs {
            header_session: None,
            headers: request.headers(),
            jwt_extractor,
            providers: &handles.repos.subject_providers,
            ctx,
        },
    )
    .await?;
    let body = axum::body::to_bytes(
        request.into_body(),
        systemprompt_models::net::INFERENCE_BODY_LIMIT_BYTES,
    )
    .await
    .unwrap_or_default();
    Ok(estimate_input_tokens(&body))
}

// JSON: Anthropic Messages count_tokens body — only its text is measured.
#[must_use]
pub fn estimate_input_tokens(body: &[u8]) -> usize {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return 0;
    };
    let chars: usize = ["system", "messages", "tools"]
        .iter()
        .filter_map(|field| value.get(*field))
        .map(text_chars)
        .sum();
    chars.div_ceil(CHARS_PER_TOKEN)
}

// JSON: a Messages body field — strings anywhere inside it are counted.
fn text_chars(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => text.chars().count(),
        serde_json::Value::Array(items) => items.iter().map(text_chars).sum(),
        serde_json::Value::Object(map) => map.values().map(text_chars).sum(),
        _ => 0,
    }
}

fn json_response(status: StatusCode, body: impl Into<Body>) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
