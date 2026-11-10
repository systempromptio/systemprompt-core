//! The headers a proxied request carries to a managed backend.
//!
//! The proxy owns every identity and context header the backend trusts. A
//! managed MCP server receives only the protocol headers in
//! [`MCP_PASSTHROUGH_HEADERS`] copied from the client, plus the headers the
//! proxy sets from the verified request context. An agent backend receives the
//! client's headers with every proxy-owned name removed before the proxy sets
//! its own, so a client cannot supply `x-proxy-verified`, `x-user-*`, the
//! owner-hop marker or a context id that the backend would believe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, HeaderName};
use systemprompt_identifiers::headers;
use systemprompt_models::RequestContext;
use systemprompt_traits::InjectContextHeaders;

pub const MCP_PASSTHROUGH_HEADERS: [&str; 5] = [
    "content-type",
    "accept",
    "mcp-session-id",
    "mcp-protocol-version",
    "last-event-id",
];

const PROXY_OWNED_HEADERS: [&str; 20] = [
    headers::AUTHORIZATION,
    headers::PROXY_VERIFIED,
    headers::USER_PERMISSIONS,
    headers::USER_ROLES,
    headers::USER_ID,
    headers::USER_TYPE,
    headers::SESSION_ID,
    headers::TRACE_ID,
    headers::CONTEXT_ID,
    headers::AGENT_NAME,
    headers::TASK_ID,
    headers::AI_TOOL_CALL_ID,
    headers::CALL_SOURCE,
    headers::CLIENT_ID,
    headers::OWNER_HOP,
    headers::CALLER_PURPOSE,
    headers::MCP_SERVER,
    headers::CLIENT_ATTESTATION,
    "x-mcp-proxy-auth",
    "host",
];

pub fn managed_mcp_headers(incoming: &HeaderMap, context: &RequestContext) -> HeaderMap {
    let mut outbound = HeaderMap::new();
    for name in MCP_PASSTHROUGH_HEADERS {
        if let Some(value) = incoming.get(name) {
            outbound.insert(HeaderName::from_static(name), value.clone());
        }
    }
    context.inject_headers(&mut outbound);
    outbound
}

pub fn agent_headers(incoming: &HeaderMap, context: &RequestContext) -> HeaderMap {
    let mut outbound = incoming.clone();
    for name in PROXY_OWNED_HEADERS {
        outbound.remove(name);
    }
    let scoped: Vec<HeaderName> = outbound
        .keys()
        .filter(|name| {
            let name = name.as_str();
            name.starts_with(headers::SCOPE_PREFIX)
                || name.starts_with("x-proxy-")
                || name.starts_with("x-user-")
        })
        .cloned()
        .collect();
    for name in scoped {
        outbound.remove(name);
    }
    context.inject_headers(&mut outbound);
    outbound
}
