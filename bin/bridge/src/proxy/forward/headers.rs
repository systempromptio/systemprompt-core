//! Header hygiene for proxied requests: hop-by-hop stripping and auth stamping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use hyper::HeaderMap;
use systemprompt_identifiers::{GatewayConversationId, SessionId, headers as sp_headers};

use super::{ForwardError, ForwardResult};

const HOP_BY_HOP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "content-length",
    "authorization",
    "x-api-key",
];

pub(super) fn build_upstream_headers(
    src: &HeaderMap,
    bearer: &str,
    session_id: &SessionId,
    gateway_conversation_id: Option<&GatewayConversationId>,
    extra: &BTreeMap<String, String>,
) -> ForwardResult<HeaderMap> {
    let mut headers = HeaderMap::with_capacity(src.len() + 4 + extra.len());
    copy_request_headers(src, &mut headers);

    let bearer = reqwest::header::HeaderValue::try_from(format!("Bearer {bearer}"))
        .map_err(|e| ForwardError::BadHeader(format!("authorization: {e}")))?;
    headers.insert(reqwest::header::AUTHORIZATION, bearer);
    headers.insert(
        reqwest::header::HeaderName::from_static("x-systemprompt-bridge"),
        reqwest::header::HeaderValue::from_static("1"),
    );
    let session_value = reqwest::header::HeaderValue::try_from(session_id.as_str())
        .map_err(|e| ForwardError::BadHeader(format!("{}: {e}", sp_headers::SESSION_ID)))?;
    headers.insert(
        reqwest::header::HeaderName::from_static(sp_headers::SESSION_ID),
        session_value,
    );
    if let Some(id) = gateway_conversation_id {
        let value = reqwest::header::HeaderValue::try_from(id.as_str()).map_err(|e| {
            ForwardError::BadHeader(format!("{}: {e}", sp_headers::GATEWAY_CONVERSATION_ID))
        })?;
        headers.insert(
            reqwest::header::HeaderName::from_static(sp_headers::GATEWAY_CONVERSATION_ID),
            value,
        );
    }

    for (k, v) in extra {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        let value = reqwest::header::HeaderValue::try_from(v)
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        headers.insert(name, value);
    }

    Ok(headers)
}

pub(super) fn copy_request_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.append(name, value);
    }
}

pub(super) fn copy_response_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            hyper::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            hyper::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.insert(name, value);
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}
