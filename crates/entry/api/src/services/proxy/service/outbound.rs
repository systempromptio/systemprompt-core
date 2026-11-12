//! The headers a proxied request carries to a managed backend.
//!
//! The proxy owns every identity and context header the backend sees. Both
//! backends receive only an allow-list of protocol headers copied from the
//! client ([`MCP_PASSTHROUGH_HEADERS`], [`AGENT_PASSTHROUGH_HEADERS`]) plus the
//! headers the proxy sets from the verified request context, so nothing else
//! a client sends (identity headers, the owner-hop marker, a context id)
//! reaches the backend. A managed MCP server additionally receives the signed
//! hop token ([`headers::MCP_HOP`]) that is the only identity it accepts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_identifiers::headers;
use systemprompt_models::RequestContext;
use systemprompt_traits::InjectContextHeaders;

pub const MCP_PASSTHROUGH_HEADERS: [&str; 5] = [
    "content-type",
    "accept",
    headers::MCP_SESSION_ID,
    headers::MCP_PROTOCOL_VERSION,
    "last-event-id",
];

pub const TRACE_PASSTHROUGH_HEADERS: [&str; 2] = ["traceparent", "tracestate"];

pub const AGENT_PASSTHROUGH_HEADERS: [&str; 7] = [
    "content-type",
    "accept",
    "last-event-id",
    headers::A2A_VERSION,
    "a2a-extensions",
    "user-agent",
    "accept-language",
];

pub fn managed_mcp_headers(
    incoming: &HeaderMap,
    context: &RequestContext,
    hop_token: &HeaderValue,
) -> HeaderMap {
    let mut outbound = allowed(incoming, &MCP_PASSTHROUGH_HEADERS);
    outbound.extend(allowed(incoming, &TRACE_PASSTHROUGH_HEADERS));
    context.inject_headers(&mut outbound);
    outbound.insert(HeaderName::from_static(headers::MCP_HOP), hop_token.clone());
    outbound
}

pub fn agent_headers(incoming: &HeaderMap, context: &RequestContext) -> HeaderMap {
    let mut outbound = allowed(incoming, &AGENT_PASSTHROUGH_HEADERS);
    outbound.extend(allowed(incoming, &TRACE_PASSTHROUGH_HEADERS));
    context.inject_headers(&mut outbound);
    outbound
}

fn allowed(incoming: &HeaderMap, names: &[&'static str]) -> HeaderMap {
    let mut outbound = HeaderMap::new();
    for name in names {
        for value in incoming.get_all(*name) {
            outbound.append(HeaderName::from_static(name), value.clone());
        }
    }
    outbound
}
