//! Gateway wire-contract tests for the Anthropic byte-passthrough lane.
//!
//! Anthropic's gateway protocol requires an `ANTHROPIC_BASE_URL` gateway to
//! forward `anthropic-*` request headers and request body fields to the
//! upstream unchanged, and to relay upstream errors without re-wrapping them.
//! Each test here asserts one clause of that contract against a mock upstream
//! that records what actually arrived, so a regression shows up as a failed
//! assertion rather than as a silently disabled capability in the field.

use std::collections::HashMap;

use futures_util::{FutureExt, StreamExt};
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// Builds the request body then sends it, mirroring what the gateway does.
///
/// The adapter splits the two so the gateway can inspect the exact bytes before
/// they go on the wire; these tests exercise the pair together.
async fn send_via<A: OutboundAdapter>(
    adapter: &A,
    ctx: OutboundCtx<'_>,
) -> anyhow::Result<OutboundOutcome> {
    let body = adapter.build_body(&ctx)?;
    adapter.send(ctx, &body).await
}

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

fn ok_body() -> Value {
    json!({
        "id": "msg_a",
        "type": "message",
        "role": "assistant",
        "model": "upstream-1",
        "content": [{ "type": "text", "text": "ok" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 1, "output_tokens": 2 }
    })
}

/// A body carrying beta-gated fields the canonical model does not describe.
///
/// `context_management` and `output_config` each pair with an `anthropic-beta`
/// value; a gateway that forwards the header but drops the field produces a
/// hard `400`, which is exactly the failure this guards against.
fn raw_with_beta_fields() -> bytes::Bytes {
    bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "hi" }],
            "context_management": { "edits": [{ "type": "clear_tool_uses_20250919" }] },
            "output_config": { "effort": "high" },
            "tools": [{ "name": "t", "input_schema": {}, "strict": true }]
        }))
        .expect("serialize"),
    )
}

async fn capture_upstream(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(response)
        .mount(server)
        .await;
}

fn last_request(server: &MockServer) -> Request {
    server
        .received_requests()
        .now_or_never()
        .flatten()
        .and_then(|mut r| r.pop())
        .expect("upstream received a request")
}

#[tokio::test]
async fn passthrough_forwards_unmodelled_body_fields_verbatim() {
    let server = MockServer::start().await;
    capture_upstream(&server, ResponseTemplate::new(200).set_body_json(ok_body())).await;

    let route = route();
    let req = request(false);
    let raw = raw_with_beta_fields();
    let outcome = send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &[],
            raw_body: Some(&raw),
        },
    )
    .await
    .expect("send");

    assert!(
        matches!(outcome, OutboundOutcome::RawBuffered { .. }),
        "a matching wire protocol must take the passthrough lane"
    );

    let sent: Value = serde_json::from_slice(&last_request(&server).body).expect("json");
    assert_eq!(
        sent["context_management"]["edits"][0]["type"],
        "clear_tool_uses_20250919"
    );
    assert_eq!(sent["output_config"]["effort"], "high");
    assert_eq!(
        sent["tools"][0]["strict"], true,
        "beta tool schema fields must survive"
    );
}

#[tokio::test]
async fn passthrough_forwards_anthropic_beta_header_verbatim() {
    let server = MockServer::start().await;
    capture_upstream(&server, ResponseTemplate::new(200).set_body_json(ok_body())).await;

    let route = route();
    let req = request(false);
    let raw = raw_with_beta_fields();
    let beta = "context-management-2025-06-27,interleaved-thinking-2025-05-14";
    let forward = vec![("anthropic-beta".to_owned(), beta.to_owned())];
    send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &forward,
            raw_body: Some(&raw),
        },
    )
    .await
    .expect("send");

    let sent = last_request(&server);
    assert_eq!(
        sent.headers
            .get("anthropic-beta")
            .expect("anthropic-beta reached the upstream"),
        beta,
        "the header must arrive unchanged, not allowlisted per value"
    );
}

#[tokio::test]
async fn client_anthropic_version_is_not_overridden() {
    let server = MockServer::start().await;
    capture_upstream(&server, ResponseTemplate::new(200).set_body_json(ok_body())).await;

    let route = route();
    let req = request(false);
    let forward = vec![("anthropic-version".to_owned(), "2099-01-01".to_owned())];
    send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &forward,
            raw_body: None,
        },
    )
    .await
    .expect("send");

    let sent = last_request(&server);
    let versions: Vec<_> = sent.headers.get_all("anthropic-version").iter().collect();
    assert_eq!(
        versions.len(),
        1,
        "the fallback version must not be sent alongside the client's"
    );
    assert_eq!(versions[0], "2099-01-01");
}

#[tokio::test]
async fn absent_client_version_falls_back_to_the_pinned_default() {
    let server = MockServer::start().await;
    capture_upstream(&server, ResponseTemplate::new(200).set_body_json(ok_body())).await;

    let route = route();
    let req = request(false);
    send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &[],
            raw_body: None,
        },
    )
    .await
    .expect("send");

    assert_eq!(
        last_request(&server)
            .headers
            .get("anthropic-version")
            .expect("a version is always sent"),
        systemprompt_models::wire::anthropic::ANTHROPIC_VERSION
    );
}

#[tokio::test]
async fn upstream_error_body_and_retry_after_are_preserved() {
    let server = MockServer::start().await;
    capture_upstream(
        &server,
        ResponseTemplate::new(529)
            .insert_header("retry-after", "42")
            .set_body_json(json!({
                "type": "error",
                "error": { "type": "overloaded_error", "message": "Overloaded" }
            })),
    )
    .await;

    let route = route();
    let req = request(false);
    let Err(err) = send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &[],
            raw_body: None,
        },
    )
    .await
    else {
        panic!("529 must surface as an error");
    };

    let upstream = err
        .downcast_ref::<UpstreamError>()
        .expect("carried as UpstreamError");
    let UpstreamError::Status {
        status,
        body,
        retry_after,
        ..
    } = upstream
    else {
        panic!("expected a status error");
    };
    assert_eq!(*status, 529);
    assert_eq!(retry_after.as_deref(), Some("42"));
    let parsed: Value = serde_json::from_slice(body).expect("body preserved as json");
    assert_eq!(
        parsed["error"]["type"], "overloaded_error",
        "Claude Code's retry path matches on the provider's own error type"
    );
}

#[tokio::test]
async fn passthrough_streaming_relays_frames_unchanged() {
    let server = MockServer::start().await;
    // Why: `citations_delta` is a real Anthropic SSE field the canonical event
    // model does not carry, so it only survives a byte-level relay.
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"upstream-1\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n\
               event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"citations_delta\",\"citation\":{\"url\":\"https://example.com\"}}}\n\n\
               event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    capture_upstream(
        &server,
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(sse),
    )
    .await;

    let route = route();
    let req = request(true);
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1",
            "max_tokens": 64,
            "stream": true,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .expect("serialize"),
    );
    let outcome = send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            forward_headers: &[],
            raw_body: Some(&raw),
        },
    )
    .await
    .expect("send");

    let OutboundOutcome::RawStreaming { stream, .. } = outcome else {
        panic!("expected the raw streaming lane");
    };
    let received: Vec<u8> = stream
        .map(|chunk| chunk.expect("chunk"))
        .collect::<Vec<_>>()
        .await
        .concat();
    assert_eq!(
        String::from_utf8(received).expect("utf8"),
        sse,
        "the client must receive the provider's own frames"
    );
}

#[tokio::test]
async fn passthrough_rewrites_only_the_model_when_the_route_remaps_it() {
    let server = MockServer::start().await;
    capture_upstream(&server, ResponseTemplate::new(200).set_body_json(ok_body())).await;

    let route = route();
    let req = request(false);
    let raw = raw_with_beta_fields();
    send_via(
        &AnthropicOutbound,
        OutboundCtx {
            route: &route,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "remapped-model",
            model_limits: None,
            forward_headers: &[],
            raw_body: Some(&raw),
        },
    )
    .await
    .expect("send");

    let sent: Value = serde_json::from_slice(&last_request(&server).body).expect("json");
    let original: Value = serde_json::from_slice(&raw).expect("json");
    assert_eq!(sent["model"], "remapped-model");
    for key in ["context_management", "output_config", "tools", "messages"] {
        assert_eq!(sent[key], original[key], "{key} must be untouched");
    }
}
