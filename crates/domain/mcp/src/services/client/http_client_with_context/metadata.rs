//! `_meta` stamping for outbound MCP requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::{HeaderName, HeaderValue};
use rmcp::model::{ClientCapabilities, ClientJsonRpcMessage, GetMeta, ProtocolVersion};
use std::collections::HashMap;
use std::hash::BuildHasher;

pub fn stamp_request_metadata<S: BuildHasher>(
    message: &mut ClientJsonRpcMessage,
    custom_headers: &HashMap<HeaderName, HeaderValue, S>,
    client_capabilities: &ClientCapabilities,
) {
    let ClientJsonRpcMessage::Request(request) = message else {
        return;
    };
    let Some(negotiated) = custom_headers
        .get(&HeaderName::from_static(HEADER_MCP_PROTOCOL_VERSION_LOWER))
        .and_then(|value| value.to_str().ok())
    else {
        return;
    };
    let Some(version) = ProtocolVersion::KNOWN_VERSIONS
        .iter()
        .find(|known| known.as_str() == negotiated)
    else {
        return;
    };
    if version.has_initialize() {
        return;
    }

    let meta = request.request.get_meta_mut();
    if meta.protocol_version().is_none() {
        meta.set_protocol_version(version.clone());
    }
    if meta.client_capabilities().is_none() {
        meta.set_client_capabilities(client_capabilities.clone());
    }
}

// Why: `HeaderName::from_static` panics on uppercase bytes; rmcp's constant
// uses mixed case.
const HEADER_MCP_PROTOCOL_VERSION_LOWER: &str = "mcp-protocol-version";
