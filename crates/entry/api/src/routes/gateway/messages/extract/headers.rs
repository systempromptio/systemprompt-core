//! Header and body extraction for inbound gateway requests.
//!
//! `x-session-id` is optional and never an error: the credential decides
//! which session a request is bound to, so an absent or unreadable header
//! leaves the choice to it.
//!
//! The systemprompt credential is read from `x-systemprompt-authorization`
//! when that header is present, and otherwise from `Authorization` or
//! `x-api-key`. A request that names it in the dedicated header keeps
//! `Authorization` for the upstream: it is never read as identity, and it is
//! the only credential a caller-credential hop presents.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_ai::PresentedCredential;
use systemprompt_identifiers::headers::{
    API_KEY, AUTHORIZATION, GATEWAY_CONVERSATION_ID, MEMORY, SESSION_ID,
    SYSTEMPROMPT_AUTHORIZATION, TRACEPARENT,
};
use systemprompt_identifiers::{GatewayConversationId, SessionId};
use systemprompt_models::origin::MemoryPreference;
use systemprompt_security::credential::{AuthHeader, AuthScheme};
use systemprompt_wire::anthropic as wire_anthropic;

use super::RejectionPartial;
use crate::kernel::{BodyReadError, read_body_bounded};
use crate::routes::gateway::messages::error::RejectionError;
use systemprompt_gateway::protocol::canonical::CanonicalRequest;
use systemprompt_gateway::protocol::inbound::InboundAdapter;

pub fn optional_session_id(headers: &HeaderMap) -> Result<Option<SessionId>, RejectionError> {
    let Some(raw) = headers.get(SESSION_ID) else {
        return Ok(None);
    };
    let raw = raw
        .to_str()
        .map_err(|e| invalid_header(SESSION_ID).with_cause(e))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    SessionId::try_new(trimmed)
        .map(Some)
        .map_err(|e| invalid_header(SESSION_ID).with_cause(e))
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

const BEARER_PREFIX: &str = "Bearer ";

fn invalid_header(name: &str) -> RejectionError {
    RejectionError::client(StatusCode::BAD_REQUEST, format!("invalid {name} header"))
}

pub async fn read_gateway_body(
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<(Bytes, CanonicalRequest), RejectionError> {
    let body_bytes = read_body_bounded(
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

pub fn body_read_error(error: BodyReadError) -> RejectionError {
    if error.is_too_large() {
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
    pub caller_auth: Option<PresentedCredential>,
}

pub fn memory_preference(headers: &HeaderMap) -> MemoryPreference {
    MemoryPreference::from_header(headers.get(MEMORY).and_then(|value| value.to_str().ok()))
}

pub(super) fn classify_client_headers(headers: &HeaderMap) -> ClientHeaders {
    let mut classified = ClientHeaders {
        memory: memory_preference(headers),
        caller_auth: presented_credential(headers),
        ..ClientHeaders::default()
    };
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
    let raw = match headers.get(SYSTEMPROMPT_AUTHORIZATION) {
        Some(dedicated) => dedicated.to_str().ok()?,
        None => headers
            .get(AUTHORIZATION)
            .or_else(|| headers.get(API_KEY))
            .and_then(|v| v.to_str().ok())?,
    };

    let trimmed = raw.strip_prefix(BEARER_PREFIX).unwrap_or(raw).trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn caller_auth(headers: &HeaderMap) -> Option<AuthHeader> {
    headers.get(SYSTEMPROMPT_AUTHORIZATION)?;
    let token = headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix(BEARER_PREFIX)?
        .trim();
    (!token.is_empty()).then(|| AuthHeader {
        scheme: AuthScheme::Bearer,
        value: token.to_owned(),
    })
}

pub fn presented_credential(headers: &HeaderMap) -> Option<PresentedCredential> {
    let auth = caller_auth(headers)?;
    let presented = headers
        .iter()
        .filter(|(name, _)| systemprompt_wire::upstream::is_presented_request_header(name.as_str()))
        .filter_map(|(name, value)| {
            Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
        })
        .collect();
    Some(PresentedCredential {
        auth,
        headers: presented,
    })
}

pub(super) fn require_credential(headers: &HeaderMap) -> Result<String, RejectionError> {
    extract_credential(headers).ok_or_else(|| {
        RejectionError::client(
            StatusCode::UNAUTHORIZED,
            "Missing Authorization, x-api-key or x-systemprompt-authorization credential",
        )
    })
}
