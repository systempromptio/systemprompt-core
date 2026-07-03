//! Header and body extraction for inbound gateway requests.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_identifiers::{GatewayConversationId, SessionId};

use super::RejectionPartial;
use crate::services::gateway::protocol::canonical::CanonicalRequest;
use crate::services::gateway::protocol::inbound::InboundAdapter;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn require_session_id(headers: &HeaderMap) -> Result<SessionId, (StatusCode, String)> {
    require_typed_header(headers, SESSION_ID, SessionId::new)
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn optional_gateway_conversation_id(
    headers: &HeaderMap,
) -> Result<Option<GatewayConversationId>, (StatusCode, String)> {
    let Some(raw) = headers.get(GATEWAY_CONVERSATION_ID) else {
        return Ok(None);
    };
    let raw = raw.to_str().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid {} header: {e}", GATEWAY_CONVERSATION_ID),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    GatewayConversationId::try_new(trimmed.to_owned())
        .map(Some)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid {} header: {e}", GATEWAY_CONVERSATION_ID),
            )
        })
}

fn require_typed_header<T>(
    headers: &HeaderMap,
    name: &'static str,
    ctor: fn(String) -> T,
) -> Result<T, (StatusCode, String)> {
    let raw = headers
        .get(name)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("missing required {name} header"),
            )
        })?
        .to_str()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid {name} header: {e}"),
            )
        })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("empty {name} header")));
    }
    Ok(ctor(trimmed.to_owned()))
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub async fn read_gateway_body(
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<(Bytes, CanonicalRequest), (StatusCode, String)> {
    let body_bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {e}"),
            )
        })?;
    partial.body = Some(body_bytes.clone());

    let canonical = inbound.parse_request(&body_bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid request body: {e}"),
        )
    })?;
    partial.model = Some(canonical.model.clone());
    partial.max_tokens = Some(canonical.max_tokens);
    partial.is_streaming = canonical.stream;
    Ok((body_bytes, canonical))
}

pub fn extract_credential(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("authorization")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok())?;

    let trimmed = raw.trim_start_matches("Bearer ").trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
