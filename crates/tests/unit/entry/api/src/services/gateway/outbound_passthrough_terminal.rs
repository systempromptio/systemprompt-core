//! The terminal-signal invariant on the Anthropic byte-passthrough lane.
//!
//! Passthrough relays the upstream body unparsed, which is what keeps the
//! client-visible payload byte-faithful. The one thing it may not relay is a
//! body that contradicts itself: a reply carrying a `tool_use` block under a
//! generic `stop_reason` ends the turn at every conforming client and the call
//! is never run. These tests pin both halves -- the contradiction is corrected
//! in the one token that is wrong, and a consistent body is byte-identical.

use std::collections::HashMap;

use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn route() -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new("anthropic"),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    }
}

fn request(stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".into())],
        }],
        max_tokens: 64,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

fn raw_request(stream: bool) -> Bytes {
    Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1",
            "max_tokens": 64,
            "stream": stream,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    )
}

fn body_with_tool_use(stop_reason: &str) -> Value {
    json!({
        "id": "msg_a",
        "type": "message",
        "role": "assistant",
        "model": "upstream-1",
        "content": [
            { "type": "text", "text": "ok" },
            { "type": "tool_use", "id": "tu_1", "name": "t", "input": { "a": 1 } }
        ],
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": { "input_tokens": 1, "output_tokens": 2 }
    })
}

fn sse_stream(stop_reason: &str) -> String {
    let frames = [
        json!({ "type": "message_start", "message": { "id": "msg_a", "content": [] } }),
        json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": { "type": "tool_use", "id": "tu_1", "name": "t", "input": {} }
        }),
        json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": { "type": "input_json_delta", "partial_json": "{\"a\":1}" }
        }),
        json!({ "type": "content_block_stop", "index": 0 }),
        json!({
            "type": "message_delta",
            "delta": { "stop_reason": stop_reason, "stop_sequence": null },
            "usage": { "output_tokens": 2 }
        }),
        json!({ "type": "message_stop" }),
    ];
    frames
        .iter()
        .map(|f| format!("event: {}\ndata: {f}\n\n", f["type"].as_str().unwrap_or("")))
        .collect()
}

async fn relay_buffered(upstream: &Value) -> Bytes {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream))
        .mount(&server)
        .await;
    let route = route();
    let req = request(false);
    let raw = raw_request(false);
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        api_key_is_bearer: false,
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let prepared = AnthropicOutbound.build_body(&ctx).expect("build");
    match AnthropicOutbound.send(ctx, &prepared).await.expect("send") {
        OutboundOutcome::RawBuffered { body, .. } => body,
        _ => panic!("expected the passthrough lane"),
    }
}

async fn relay_streaming(sse: &str) -> String {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;
    let route = route();
    let req = request(true);
    let raw = raw_request(true);
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        api_key_is_bearer: false,
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let prepared = AnthropicOutbound.build_body(&ctx).expect("build");
    let OutboundOutcome::RawStreaming { mut stream, .. } =
        AnthropicOutbound.send(ctx, &prepared).await.expect("send")
    else {
        panic!("expected the passthrough streaming lane");
    };
    let mut out = Vec::new();
    while let Some(chunk) = stream.next().await {
        out.extend_from_slice(&chunk.expect("chunk"));
    }
    String::from_utf8(out).expect("utf8")
}

#[tokio::test]
async fn buffered_generic_stop_beside_a_tool_use_block_is_corrected() {
    let relayed = relay_buffered(&body_with_tool_use("end_turn")).await;
    let expected = serde_json::to_vec(&body_with_tool_use("tool_use")).expect("serialize");
    assert_eq!(
        relayed.as_ref(),
        expected.as_slice(),
        "only the stop_reason token may differ from the upstream body"
    );
}

#[tokio::test]
async fn buffered_consistent_body_is_relayed_byte_for_byte() {
    let upstream = body_with_tool_use("tool_use");
    let relayed = relay_buffered(&upstream).await;
    let expected = serde_json::to_vec(&upstream).expect("serialize");
    assert_eq!(relayed.as_ref(), expected.as_slice());
}

#[tokio::test]
async fn buffered_truncation_still_wins_over_the_tool_call() {
    let upstream = body_with_tool_use("max_tokens");
    let relayed = relay_buffered(&upstream).await;
    let expected = serde_json::to_vec(&upstream).expect("serialize");
    assert_eq!(relayed.as_ref(), expected.as_slice());
}

#[tokio::test]
async fn streaming_generic_stop_beside_a_tool_use_block_is_corrected() {
    let relayed = relay_streaming(&sse_stream("end_turn")).await;
    assert_eq!(relayed, sse_stream("tool_use"));
}

#[tokio::test]
async fn streaming_consistent_stream_is_relayed_byte_for_byte() {
    let sse = sse_stream("tool_use");
    assert_eq!(relay_streaming(&sse).await, sse);
}

#[tokio::test]
async fn streaming_truncation_still_wins_over_the_tool_call() {
    let sse = sse_stream("max_tokens");
    assert_eq!(relay_streaming(&sse).await, sse);
}
