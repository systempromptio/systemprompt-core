//! Header and body extraction for inbound gateway requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_identifiers::{GatewayConversationId, SessionId};
use systemprompt_models::wire::anthropic as wire_anthropic;

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
    let body_bytes = axum::body::to_bytes(
        request.into_body(),
        systemprompt_models::wire::BUFFERED_BODY_LIMIT_BYTES,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to read request body: {e}"),
        )
    })?;
    partial.body = Some(body_bytes.clone());

    let canonical = inbound.parse_request(&body_bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid request body: {e}"),
        )
    })?;
    partial.model = Some(canonical.model.clone());
    partial.max_tokens = Some(canonical.max_tokens);
    partial.is_streaming = canonical.stream;
    Ok((body_bytes, canonical))
}

// Why: `forward` is relayed upstream unchanged; `identity` is recorded on the
// audit row and dropped, so a third-party provider never sees which developer,
// session, or agent produced the request. Credential-bearing identity headers
// keep their name and lose their value (`recordable_header_value`) — the vec is
// logged, and it is never the thing that needs the secret.
#[derive(Debug, Default, Clone)]
pub(crate) struct ClientHeaders {
    pub forward: Vec<(String, String)>,
    pub identity: Vec<(String, String)>,
}

pub(super) fn classify_client_headers(headers: &HeaderMap) -> ClientHeaders {
    let mut classified = ClientHeaders::default();
    for (name, value) in headers {
        let Ok(value) = value.to_str() else {
            continue;
        };
        let name = name.as_str();
        if wire_anthropic::is_forwardable_request_header(name) {
            classified.forward.push((name.to_owned(), value.to_owned()));
        } else if wire_anthropic::is_identity_request_header(name) {
            // Why: the identity vec is recorded on the audit row and logged, so
            // a credential header contributes its name but never its value.
            classified.identity.push((
                name.to_owned(),
                wire_anthropic::recordable_header_value(name, value),
            ));
        }
    }
    classified
}

pub fn extract_credential(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("authorization")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok())?;

    let trimmed = raw.strip_prefix("Bearer ").unwrap_or(raw).trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
