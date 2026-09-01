//! `_meta` stamping for outbound MCP requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::{HeaderName, HeaderValue};
use rmcp::model::{ClientCapabilities, ClientJsonRpcMessage, GetMeta, ProtocolVersion};
use std::collections::HashMap;

// Why: SEP-2575. From 2026-07-28 a stateless server rejects any non-initialize
// request whose `_meta` omits the negotiated protocol version and client
// capabilities ("request _meta is missing or has malformed required fields").
// rmcp sets the `MCP-Protocol-Version` header but never the `_meta` fields,
// so without this every call fails at the transport. Below 2026-07-28 nothing
// is stamped. The SEP-2243 headers need no help: rmcp adds them itself.
pub(super) fn stamp_request_metadata(
    message: &mut ClientJsonRpcMessage,
    custom_headers: &HashMap<HeaderName, HeaderValue>,
    client_capabilities: &ClientCapabilities,
) {
    let ClientJsonRpcMessage::Request(request) = message else {
        return;
    };
    // Why: The negotiated version, as rmcp resolved it at `initialize` and now
    // echoes on every request. Reading it back rather than assuming a version
    // is what keeps an older or third-party server unaffected, and it
    // guarantees the header and the `_meta` field agree — the server rejects a
    // mismatch as loudly as it rejects an omission.
    let Some(negotiated) = custom_headers
        .get(&HeaderName::from_static(HEADER_MCP_PROTOCOL_VERSION_LOWER))
        .and_then(|value| value.to_str().ok())
    else {
        return;
    };
    if negotiated < ProtocolVersion::V_2026_07_28.as_str() {
        return;
    }
    // Why: resolved against the SDK's own list rather than reconstructed from
    // the string. A version this client does not know is one whose `_meta`
    // contract it cannot claim to satisfy, so it is left alone.
    let Some(version) = ProtocolVersion::KNOWN_VERSIONS
        .iter()
        .find(|known| known.as_str() == negotiated)
    else {
        return;
    };

    let meta = request.request.get_meta_mut();
    if meta.protocol_version().is_none() {
        meta.set_protocol_version(version.clone());
    }
    if meta.client_capabilities().is_none() {
        meta.set_client_capabilities(client_capabilities.clone());
    }
}

// Why: `HeaderName::from_static` panics on an uppercase byte, and rmcp's
// constant is the canonical mixed-case spelling.
const HEADER_MCP_PROTOCOL_VERSION_LOWER: &str = "mcp-protocol-version";
