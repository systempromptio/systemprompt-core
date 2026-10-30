//! Header and body extraction for inbound gateway requests.
//!
//! `x-session-id` is optional and never an error: the credential decides
//! which session a request is bound to, so an absent or unreadable header
//! leaves the choice to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, MEMORY, SESSION_ID, TRACEPARENT};
use systemprompt_identifiers::{GatewayConversationId, SessionId};
use systemprompt_models::origin::MemoryPreference;
use systemprompt_wire::anthropic as wire_anthropic;

use super::RejectionPartial;
use crate::routes::gateway::messages::error::RejectionError;
use systemprompt_gateway::protocol::canonical::CanonicalRequest;
use systemprompt_gateway::protocol::inbound::InboundAdapter;

pub fn optional_session_id(headers: &HeaderMap) -> Option<SessionId> {
    headers
        .get(SESSION_ID)
        .and_then(|raw| raw.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| SessionId::new(value.to_owned()))
}

pub fn optional_gateway_conversation_id(
    headers: &HeaderMap,
) -> Result<Option<GatewayConversationId>, RejectionError> {
    let Some(raw) = headers.get(GATEWAY_CONVERSATION_ID) else {
        return Ok(None);
    };
    let raw = raw
        .to_str()
        .map_err(|e| invalid_header(GATEWAY_CONVERSATION_ID).with_cause(e))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    GatewayConversationId::try_new(trimmed.to_owned())
        .map(Some)
        .map_err(|e| invalid_header(GATEWAY_CONVERSATION_ID).with_cause(e))
}

fn invalid_header(name: &str) -> RejectionError {
    RejectionError::client(StatusCode::BAD_REQUEST, format!("invalid {name} header"))
}

pub async fn read_gateway_body(
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<(Bytes, CanonicalRequest), RejectionError> {
    let body_bytes = axum::body::to_bytes(
        request.into_body(),
        systemprompt_models::net::INFERENCE_BODY_LIMIT_BYTES,
    )
    .await
    .map_err(body_read_error)?;
    partial.body = Some(body_bytes.clone());

    let canonical = inbound
        .parse_request(&body_bytes)
        .map_err(|e| RejectionError::invalid(StatusCode::BAD_REQUEST, e))?;
    partial.model = Some(canonical.model.to_string());
    partial.max_tokens = Some(canonical.max_tokens);
    partial.is_streaming = canonical.stream;
    Ok((body_bytes, canonical))
}

pub fn body_read_error(error: axum::Error) -> RejectionError {
    let oversized = std::error::Error::source(&error)
        .is_some_and(<dyn std::error::Error>::is::<http_body_util::LengthLimitError>);
    if oversized {
        RejectionError::client(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "serialized inference request exceeds {} bytes; model context capacity is a separate token limit",
                systemprompt_models::net::INFERENCE_BODY_LIMIT_BYTES
            ),
        )
        .with_cause(error)
    } else {
        RejectionError::client(StatusCode::BAD_REQUEST, "failed to read request body")
            .with_cause(error)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ClientHeaders {
    pub forward: Vec<(String, String)>,
    pub identity: Vec<(String, String)>,
    pub traceparent: Option<String>,
    pub memory: MemoryPreference,
}

pub fn memory_preference(headers: &HeaderMap) -> MemoryPreference {
    MemoryPreference::from_header(headers.get(MEMORY).and_then(|value| value.to_str().ok()))
}

pub(super) fn classify_client_headers(headers: &HeaderMap) -> ClientHeaders {
    let mut classified = ClientHeaders::default();
    classified.memory = memory_preference(headers);
    for (name, value) in headers {
        let Ok(value) = value.to_str() else {
            continue;
        };
        let name = name.as_str();
        if name == TRACEPARENT {
            classified.traceparent = Some(value.to_owned());
        }
        if wire_anthropic::is_forwardable_request_header(name) {
            classified.forward.push((name.to_owned(), value.to_owned()));
        } else if wire_anthropic::is_identity_request_header(name) {
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

pub(super) fn require_credential(headers: &HeaderMap) -> Result<String, RejectionError> {
    extract_credential(headers).ok_or_else(|| {
        RejectionError::client(
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential",
        )
    })
}
