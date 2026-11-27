//! The route handlers, and what they share: reading a credential and
//! refusing a request.

pub(crate) mod auth;
pub(crate) mod hooks;
pub(crate) mod model;
pub(crate) mod sync;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value};
use systemprompt_identifiers::headers as names;
use systemprompt_models::bridge::envelope::ErrorEnvelope;

use crate::state::InstanceState;
use crate::token::{HOOK_USE, SESSION_USE};

const BEARER: &str = "Bearer ";

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(names::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_at_checked(BEARER.len())?;
    scheme.eq_ignore_ascii_case(BEARER).then_some(token.trim())
}

pub(crate) fn refuse(status: StatusCode, message: &str) -> Response {
    let envelope = ErrorEnvelope {
        code: Some(status.as_u16().to_string()),
        message: Some(message.to_owned()),
        error_key: None,
        timestamp: None,
    };
    (status, Json(envelope)).into_response()
}

pub(crate) fn unauthorized(what: &str) -> Response {
    refuse(StatusCode::UNAUTHORIZED, what)
}

pub(crate) fn session(
    state: &InstanceState,
    headers: &HeaderMap,
) -> Result<Map<String, Value>, Response> {
    bearer(headers)
        .and_then(|token| state.keys.claims(SESSION_USE, token))
        .ok_or_else(|| unauthorized("a session token this instance issued is required"))
}

pub(crate) fn hook(state: &InstanceState, headers: &HeaderMap) -> Result<(), Response> {
    bearer(headers)
        .and_then(|token| state.keys.claims(HOOK_USE, token))
        .map(|_claims| ())
        .ok_or_else(|| unauthorized("a hook token this instance issued is required"))
}

pub(crate) fn holds_model_credential(state: &InstanceState, headers: &HeaderMap) -> bool {
    let api_key = headers
        .get(names::API_KEY)
        .and_then(|value| value.to_str().ok());
    bearer(headers).or(api_key).is_some_and(|credential| {
        credential == state.pat.as_str() || state.keys.claims(SESSION_USE, credential).is_some()
    })
}
