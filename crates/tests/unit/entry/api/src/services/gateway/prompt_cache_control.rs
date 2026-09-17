//! Prompt-caching contract for the Anthropic lane.
//!
//! Anthropic's `cache_control` breakpoints ride on individual blocks. The
//! same-wire passthrough lane forwards the client's bytes, so the prepared
//! body is the request body; the rebuild lane (system-prompt override,
//! cross-wire translation) goes through the canonical model and must put every
//! breakpoint back exactly where the client set it.

use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::audit::payload::digest_hex;
use systemprompt_api::services::gateway::protocol::canonical::{
    CacheControl, CacheTtl, CanonicalContent, CanonicalRequest,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::outbound::anthropic::{
    AnthropicOutbound, request,
};
use systemprompt_api::services::gateway::protocol::outbound::{OutboundAdapter, OutboundCtx};
use systemprompt_models::services::GatewayRoute;

/// The shape Claude Code sends: breakpoints on the last system block, the last
/// tool, and the last block of the newest user turn.
fn cached_body() -> Value {
    json!({
        "model": "claude-x",
        "max_tokens": 64,
        "system": [
            { "type": "text", "text": "stable preamble" },
            { "type": "text", "text": "cached tail", "cache_control": { "type": "ephemeral", "ttl": "1h" } }
        ],
        "tools": [
            { "name": "read", "description": "read a file", "input_schema": { "type": "object", "properties": {} }, "cache_control": { "type": "ephemeral" } }
        ],
        "messages": [
            { "role": "user", "content": [
                { "type": "text", "text": "open it" },
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AAA=" } }
            ] },
            { "role": "assistant", "content": [
                { "type": "tool_use", "id": "tu_1", "name": "read", "input": { "path": "/a" } }
            ] },
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "tu_1", "is_error": false,
                  "content": [ { "type": "text", "text": "42" } ],
                  "cache_control": { "type": "ephemeral" } }
            ] }
        ]
    })
}

fn parse(body: &Value) -> CanonicalRequest {
    let bytes = Bytes::from(serde_json::to_vec(body).expect("serialize"));
    AnthropicMessagesInbound
        .parse_request(&bytes)
        .expect("a cached body is well-formed")
}

fn route() -> GatewayRoute {
    serde_json::from_value(json!({ "model_pattern": "*", "provider": "anthropic" })).expect("route")
}

#[test]
fn inbound_parse_keeps_cache_control_on_every_block_kind() {
    let req = parse(&cached_body());

    assert_eq!(req.system[0].cache_control, None);
    assert_eq!(
        req.system[1].cache_control,
        Some(CacheControl::with_ttl(CacheTtl::OneHour))
    );
    assert_eq!(req.tools[0].cache_control, Some(CacheControl::EPHEMERAL));
    assert_eq!(req.messages[0].content[0].cache_control(), None);
    assert!(matches!(
        req.messages[2].content[0],
        CanonicalContent::ToolResult {
            cache_control: Some(CacheControl::EPHEMERAL),
            ..
        }
    ));
}

#[test]
fn anthropic_rebuild_is_lossless_for_cache_control() {
    let original = cached_body();
    let req = parse(&original);

    let rebuilt = request::build_request_body(&req, "claude-x", None);

    assert_eq!(rebuilt["system"], original["system"]);
    assert_eq!(rebuilt["messages"], original["messages"]);
    assert_eq!(
        rebuilt["tools"][0]["cache_control"],
        original["tools"][0]["cache_control"]
    );
    assert_eq!(rebuilt["tools"][0]["name"], original["tools"][0]["name"]);
}

#[test]
fn a_system_prompt_override_keeps_the_message_breakpoints() {
    let mut req = parse(&cached_body());
    req.set_system_text(Some("governed prompt".to_owned()));

    let rebuilt = request::build_request_body(&req, "claude-x", None);

    assert_eq!(rebuilt["system"], "governed prompt");
    assert_eq!(
        rebuilt["messages"][2]["content"][0]["cache_control"],
        json!({ "type": "ephemeral" }),
        "replacing the system prompt must not move or drop the conversation breakpoints"
    );
    assert_eq!(
        rebuilt["tools"][0]["cache_control"],
        json!({ "type": "ephemeral" })
    );
}

#[test]
fn passthrough_prepared_body_digest_equals_request_body_digest() {
    let raw = Bytes::from(serde_json::to_vec(&cached_body()).expect("serialize"));
    let req = parse(&cached_body());
    let route = route();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: "http://unused.invalid",
        api_key: "k",
        api_key_is_bearer: false,
        request: &req,
        upstream_model: "claude-x",
        model_limits: None,
        forward_headers: &[],
        raw_body: Some(&raw),
    };

    let prepared = AnthropicOutbound.build_body(&ctx).expect("build");

    assert!(
        prepared.raw_lane,
        "a same-wire request takes the passthrough lane"
    );
    assert_eq!(
        digest_hex(&prepared.bytes),
        digest_hex(&raw),
        "prepared_body_sha256 must equal request_body_sha256 on the passthrough route"
    );
}
