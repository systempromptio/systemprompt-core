//! The gateway's core governance invariant: what is inspected is what is sent.
//!
//! The byte-passthrough lane forwards the caller's own request body so that
//! fields the canonical model does not describe still reach the provider. That
//! is a correctness win and a governance hazard: the canonical parse drops any
//! content block whose `type` it does not model (`document`, `search_result`,
//! `server_tool_use`, …), carries no text for images, and has no home for
//! `structuredContent` or `_meta`. A credential in any of those places would be
//! forwarded without a scanner ever seeing it.
//!
//! The gateway closes that by attaching every string in the prepared outbound
//! bytes to the canonical request before the safety scan runs. These tests pin
//! that: the surface is derived from the bytes that will actually be sent, and
//! it reaches the accessor blocking scanners read.

use std::collections::HashMap;

use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{OutboundAdapter, OutboundCtx};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::profile::GatewayRoute;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::inspect::{SurfaceBudget, string_leaves};

const LEAKED_KEY: &str = "AKIAIOSFODNN7EXAMPLE";

fn route() -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new("anthropic"),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
    }
}

/// A canonical request whose modelled content is entirely innocuous.
fn clean_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("summarise this".into())],
        }],
        max_tokens: 64,
        ..CanonicalRequest::default()
    }
}

/// A raw body carrying a credential only in places the canonical parse drops.
fn raw_body_hiding_a_secret() -> bytes::Bytes {
    bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1",
            "max_tokens": 64,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": "summarise this" },
                    { "type": "document", "source": { "data": LEAKED_KEY } }
                ]
            }]
        }))
        .expect("serialize"),
    )
}

fn ctx<'a>(
    route: &'a GatewayRoute,
    request: &'a CanonicalRequest,
    raw: Option<&'a bytes::Bytes>,
    limits: Option<ModelLimits>,
) -> OutboundCtx<'a> {
    OutboundCtx {
        route,
        endpoint: "http://unused.invalid",
        api_key: "k",
        request,
        upstream_model: "upstream-1",
        model_limits: limits,
        forward_headers: &[],
        raw_body: raw,
    }
}

#[test]
fn a_secret_the_canonical_parse_drops_is_still_in_the_inspection_surface() {
    let route = route();
    let request = clean_request();
    let raw = raw_body_hiding_a_secret();

    assert!(
        !request.flatten_text().contains(LEAKED_KEY),
        "precondition: the canonical form must not contain the secret, or this \
         test would pass for the wrong reason"
    );

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), None))
        .expect("build");
    assert!(prepared.raw_lane, "a matching wire must take the raw lane");

    let mut governed = request;
    governed.forwarded_surface = string_leaves(&prepared.bytes, SurfaceBudget::default());

    assert!(
        governed.flatten_text().contains(LEAKED_KEY),
        "a credential that will be forwarded must be visible to a scanner \
         reading flatten_text"
    );
    assert!(
        governed.message_units().iter().any(|u| u.contains(LEAKED_KEY)),
        "and to a scanner reading message_units"
    );
}

#[test]
fn the_inspection_surface_is_derived_from_the_bytes_that_will_be_sent() {
    let route = route();
    let request = clean_request();
    let raw = raw_body_hiding_a_secret();

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), None))
        .expect("build");
    let surface = string_leaves(&prepared.bytes, SurfaceBudget::default());

    // Why: pinning the invariant itself rather than a sample of it — every
    // string in the outgoing body must appear in what governance inspected.
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("json");
    let mut expected = Vec::new();
    collect_strings(&sent, &mut expected);
    for value in expected {
        assert!(
            surface.leaves().iter().any(|leaf| leaf.value == value),
            "string {value:?} is forwarded but absent from the inspection surface"
        );
    }
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) if !s.is_empty() => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        },
        Value::Object(map) => {
            for (key, item) in map {
                out.push(key.clone());
                collect_strings(item, out);
            }
        },
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

#[test]
fn the_raw_lane_clamps_max_tokens_down_to_the_model_ceiling() {
    let route = route();
    let request = clean_request();
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1", "max_tokens": 999_999,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    );
    let limits = ModelLimits {
        max_output_tokens: 4_096,
        ..ModelLimits::default()
    };

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), Some(limits)))
        .expect("build");
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("json");

    assert_eq!(
        sent["max_tokens"], 4_096,
        "the model's output ceiling is a cost control the raw lane must not bypass"
    );
}

#[test]
fn the_raw_lane_never_raises_a_lower_max_tokens() {
    let route = route();
    let request = clean_request();
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1", "max_tokens": 16,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    );
    let limits = ModelLimits {
        max_output_tokens: 4_096,
        ..ModelLimits::default()
    };

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), Some(limits)))
        .expect("build");
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("json");

    assert_eq!(sent["max_tokens"], 16, "the limit is a cap, not a target");
}

#[test]
fn the_raw_lane_strips_the_callers_end_user_id() {
    let route = route();
    let request = clean_request();
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1", "max_tokens": 8,
            "metadata": { "user_id": "user-abc" },
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    );

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), None))
        .expect("build");
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("json");

    assert!(
        sent.get("metadata").is_none(),
        "metadata must be dropped once it empties, not sent as {{}}: {sent}"
    );
}

#[test]
fn the_raw_lane_preserves_metadata_the_caller_set_alongside_user_id() {
    let route = route();
    let request = clean_request();
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1", "max_tokens": 8,
            "metadata": { "user_id": "user-abc", "keep": "me" },
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    );

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), None))
        .expect("build");
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("json");

    assert_eq!(sent["metadata"]["keep"], "me");
    assert!(sent["metadata"].get("user_id").is_none());
}

#[test]
fn a_raw_body_that_is_not_a_json_object_falls_back_to_the_canonical_lane() {
    let route = route();
    let request = clean_request();
    let raw = bytes::Bytes::from_static(b"not json at all");

    let prepared = AnthropicOutbound
        .build_body(&ctx(&route, &request, Some(&raw), None))
        .expect("build");

    assert!(
        !prepared.raw_lane,
        "bytes the gateway cannot parse must be rebuilt, never relayed unread"
    );
    let sent: Value = serde_json::from_slice(&prepared.bytes).expect("the rebuild is valid JSON");
    assert_eq!(sent["model"], "upstream-1");
}
