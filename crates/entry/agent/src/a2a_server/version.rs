//! A2A protocol version negotiation for the JSON-RPC endpoint.
//!
//! A client names the protocol version it speaks in the `A2A-Version` header
//! or the `A2A-Version` query parameter. [`negotiate`] accepts exactly the
//! version this server implements; a missing, empty or different version is
//! refused with `VersionNotSupportedError` (-32009). Agent-card discovery is
//! not versioned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, Uri};
use systemprompt_models::a2a::{PROTOCOL_VERSION, VERSION_HEADER};

use super::errors::JsonRpcErrorBuilder;

pub fn negotiate(headers: &HeaderMap, uri: &Uri) -> Result<(), JsonRpcErrorBuilder> {
    let requested = requested_version(headers, uri).unwrap_or_default();
    if requested == PROTOCOL_VERSION {
        return Ok(());
    }
    Err(JsonRpcErrorBuilder::version_not_supported(&requested)
        .log_warn(format!("A2A version not supported: '{requested}'")))
}

fn requested_version(headers: &HeaderMap, uri: &Uri) -> Option<String> {
    if let Some(value) = headers.get(VERSION_HEADER) {
        return Some(String::from_utf8_lossy(value.as_bytes()).trim().to_owned());
    }
    uri.query().and_then(|query| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| key.eq_ignore_ascii_case(VERSION_HEADER))
            .map(|(_, value)| value.trim().to_owned())
    })
}
