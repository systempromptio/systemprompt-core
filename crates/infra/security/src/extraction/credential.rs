//! The systemprompt credential a gateway-style request presents.
//!
//! The credential is read from `x-systemprompt-authorization` when that
//! header is present, and otherwise from `Authorization` or `x-api-key`; a
//! `Bearer ` prefix is stripped and an empty value is no credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::headers::{API_KEY, AUTHORIZATION, SYSTEMPROMPT_AUTHORIZATION};

const BEARER_PREFIX: &str = "Bearer ";

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
