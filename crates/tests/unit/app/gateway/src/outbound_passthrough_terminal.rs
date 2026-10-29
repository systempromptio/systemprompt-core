//! The terminal-signal invariant on the Anthropic byte-passthrough lane.
//!
//! Passthrough relays the upstream body unparsed, which is what keeps the
//! client-visible payload byte-faithful. The one thing it may not relay is a
//! body that contradicts itself: a reply carrying a `tool_use` block under a
//! generic `stop_reason` ends the turn at every conforming client and the call
//! is never run. These tests pin both halves -- the contradiction is corrected
//! in the one token that is wrong, and a consistent body is byte-identical.

use super::support;

use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalReasoning, CanonicalRequest, CanonicalSampling,
    CanonicalTooling, Role,
};
use systemprompt_gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_gateway::protocol::outbound::{OutboundAdapter, OutboundCtx, OutboundOutcome};
use systemprompt_identifiers::ModelId;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn route() -> systemprompt_manifest::services::catalog::ResolvedHop {
    systemprompt_catalog_fixtures::hop(
        "anthropic",
        systemprompt_wire::WireProtocol::Anthropic,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    )
}

fn request(stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        cache_control: None,
        system: Vec::new(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        max_tokens: 64,
        stream,
        metadata: None,
        response_format: None,
        forwarded_surface: Default::default(),
        sampling: CanonicalSampling {
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: vec![],
            presence_penalty: None,
            frequency_penalty: None,
        },
        tooling: CanonicalTooling {
            tools: vec![],
            tool_choice: None,
            search: None,
            code_execution: false,
        },
        reasoning: CanonicalReasoning {
            thinking: None,
            reasoning_effort: None,
        },
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
        target: &route,
        upstream: &support::api_key_call(&server.uri(), "k"),
        http: &reqwest::Client::new(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
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
        target: &route,
        upstream: &support::api_key_call(&server.uri(), "k"),
        http: &reqwest::Client::new(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
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

async fn relay_buffered_text(upstream: &str) -> Bytes {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(upstream),
        )
        .mount(&server)
        .await;
    let route = route();
    let req = request(false);
    let raw = raw_request(false);
    let ctx = OutboundCtx {
        target: &route,
        upstream: &support::api_key_call(&server.uri(), "k"),
        http: &reqwest::Client::new(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let prepared = AnthropicOutbound.build_body(&ctx).expect("build");
    match AnthropicOutbound.send(ctx, &prepared).await.expect("send") {
        OutboundOutcome::RawBuffered { body, .. } => body,
        _ => panic!("expected the passthrough lane"),
    }
}

#[tokio::test]
async fn buffered_correction_skips_lookalike_tokens_and_tolerates_whitespace() {
    let upstream = r#"{"content":[{"type":"text","text":"stop_reason"},{"type":"tool_use","id":"tu_1","name":"t","input":{"stop_reason" : "other"}}], "stop_reason" :  "end_turn", "usage":{"input_tokens":1,"output_tokens":1}}"#;
    let relayed = relay_buffered_text(upstream).await;
    let expected = upstream.replace(
        r#""stop_reason" :  "end_turn""#,
        r#""stop_reason" :  "tool_use""#,
    );
    assert_eq!(String::from_utf8_lossy(&relayed), expected);
}

#[tokio::test]
async fn buffered_bodies_without_a_stop_reason_are_untouched() {
    let no_reason = r#"{"content":[{"type":"tool_use","id":"tu_1","name":"t","input":{}}]}"#;
    assert_eq!(
        relay_buffered_text(no_reason).await.as_ref(),
        no_reason.as_bytes()
    );
}

#[tokio::test]
async fn streaming_comment_frames_and_reasonless_deltas_pass_through() {
    let mut sse = String::from(": keepalive\n\n");
    sse.push_str(&sse_stream("tool_use"));
    sse.push_str(&format!(
        "event: message_delta\ndata: {}\n\n",
        json!({ "type": "message_delta", "delta": {}, "usage": { "output_tokens": 3 } })
    ));
    sse.push_str("data: trailing");
    assert_eq!(relay_streaming(&sse).await, sse);
}
