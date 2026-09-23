//! Claude on Vertex AI through the gateway's Anthropic adapter.
//!
//! The adapter is the same one that serves `api.anthropic.com`; only the
//! upstream call's hosting differs. These tests pin the Vertex envelope: the
//! model's own `:rawPredict`/`:streamRawPredict` path, a bearer token instead
//! of `x-api-key`, `anthropic_version` in the body in place of the header, and
//! no `model` field — for the translated and the passthrough lanes alike.

use std::collections::HashMap;

use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_ai::UpstreamCall;

use super::support;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome,
};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId};
use systemprompt_models::services::{GatewayRoute, Hosting};
use systemprompt_models::wire::upstream::VERTEX_ANTHROPIC_VERSION;
use systemprompt_security::credential::{AuthHeader, AuthScheme};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "ya29.fixture";

fn vertex_call(endpoint: &str) -> UpstreamCall {
    UpstreamCall::new(
        Hosting::Vertex,
        endpoint.to_owned(),
        AuthHeader {
            scheme: AuthScheme::Bearer,
            value: TOKEN.to_owned(),
        },
        Vec::new(),
    )
}

fn route() -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("claude-vertex"),
        name: None,
        description: None,
        model_pattern: "claude-*".into(),
        provider: ProviderId::new("vertex-anthropic"),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
        fallback_provider: None,
        fallback_upstream_model: None,
    }
}

fn request(stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("claude-sonnet-5"),
        cache_control: None,
        system: Vec::new(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        max_tokens: 64,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
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

fn message() -> Value {
    json!({
        "id": "msg_v",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-5",
        "content": [{ "type": "text", "text": "from vertex" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 3, "output_tokens": 2 }
    })
}

fn received_body(server_requests: &[wiremock::Request]) -> Value {
    assert_eq!(server_requests.len(), 1);
    serde_json::from_slice(&server_requests[0].body).expect("upstream body is JSON")
}

#[tokio::test]
async fn buffered_request_posts_raw_predict_with_bearer_and_body_version() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/claude-sonnet-5@fixture:rawPredict"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(message()))
        .expect(1)
        .mount(&server)
        .await;

    let route = route();
    let req = request(false);
    let call =
        vertex_call(&server.uri()).with_accepted_betas(Some(vec!["fixture-beta".to_owned()]));
    let forwarded = [
        ("anthropic-version".to_owned(), "2023-06-01".to_owned()),
        ("anthropic-beta".to_owned(), "fixture-beta".to_owned()),
    ];
    let ctx = OutboundCtx {
        route: &route,
        upstream: &call,
        request: &req,
        upstream_model: "claude-sonnet-5@fixture",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &forwarded,
        raw_body: None,
    };
    let adapter = AnthropicOutbound;
    let body = adapter.build_body(&ctx).expect("body");
    let outcome = adapter.send(ctx, &body).await.expect("vertex answers");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));

    let requests = server.received_requests().await.expect("recorded");
    let sent = &requests[0];
    assert!(sent.headers.get("x-api-key").is_none());
    assert!(sent.headers.get("anthropic-version").is_none());
    assert_eq!(
        sent.headers
            .get("anthropic-beta")
            .map(|v| v.to_str().unwrap()),
        Some("fixture-beta")
    );
    let body = received_body(&requests);
    assert_eq!(body["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
    assert!(
        body.get("model").is_none(),
        "Vertex names the model in the URL"
    );
}

#[tokio::test]
async fn streaming_request_posts_stream_raw_predict() {
    let sse = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_s\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-sonnet-5\",\"content\":[],\"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/claude-sonnet-5@fixture:streamRawPredict"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .expect(1)
        .mount(&server)
        .await;

    let route = route();
    let req = request(true);
    let call = vertex_call(&server.uri());
    let ctx = OutboundCtx {
        route: &route,
        upstream: &call,
        request: &req,
        upstream_model: "claude-sonnet-5@fixture",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: None,
    };
    let adapter = AnthropicOutbound;
    let body = adapter.build_body(&ctx).expect("body");
    let OutboundOutcome::Streaming(mut events) = adapter.send(ctx, &body).await.expect("ok") else {
        panic!("expected a canonical stream");
    };
    let mut count = 0;
    while let Some(event) = events.next().await {
        event.expect("frame decodes");
        count += 1;
    }
    assert!(count > 0, "the shared SSE framing yields events");

    let requests = server.received_requests().await.expect("recorded");
    let body = received_body(&requests);
    assert_eq!(body["stream"], true);
    assert_eq!(body["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
    assert!(body.get("model").is_none());
}

#[test]
fn passthrough_body_drops_model_and_carries_the_vertex_version() {
    let route = route();
    let req = request(false);
    let call = vertex_call("http://unused.invalid");
    let raw = bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-5",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "hi" }]
        }))
        .unwrap(),
    );
    let ctx = OutboundCtx {
        route: &route,
        upstream: &call,
        request: &req,
        upstream_model: "claude-sonnet-5@fixture",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let prepared = AnthropicOutbound.build_body(&ctx).expect("body");
    assert!(prepared.raw_lane);
    let body: Value = serde_json::from_slice(&prepared.bytes).unwrap();
    assert!(body.get("model").is_none());
    assert_eq!(body["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
}

#[tokio::test]
async fn first_party_anthropic_keeps_messages_path_and_api_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(header("x-api-key", "sk-fixture"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(message()))
        .expect(1)
        .mount(&server)
        .await;

    let route = route();
    let req = request(false);
    let call = support::api_key_call(&server.uri(), "sk-fixture");
    let ctx = OutboundCtx {
        route: &route,
        upstream: &call,
        request: &req,
        upstream_model: "claude-sonnet-5",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: None,
    };
    let adapter = AnthropicOutbound;
    let body = adapter.build_body(&ctx).expect("body");
    adapter.send(ctx, &body).await.expect("first party answers");
    let body = received_body(&server.received_requests().await.unwrap());
    assert_eq!(body["model"], "claude-sonnet-5");
    assert!(body.get("anthropic_version").is_none());
}

#[tokio::test]
async fn an_undeclared_beta_is_not_forwarded_to_vertex() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/claude-sonnet-5@fixture:rawPredict"))
        .respond_with(ResponseTemplate::new(200).set_body_json(message()))
        .expect(1)
        .mount(&server)
        .await;

    let route = route();
    let req = request(false);
    let call = vertex_call(&server.uri());
    let forwarded = [("anthropic-beta".to_owned(), "fixture-beta".to_owned())];
    let ctx = OutboundCtx {
        route: &route,
        upstream: &call,
        request: &req,
        upstream_model: "claude-sonnet-5@fixture",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &forwarded,
        raw_body: None,
    };
    let adapter = AnthropicOutbound;
    let body = adapter.build_body(&ctx).expect("body");
    adapter.send(ctx, &body).await.expect("vertex answers");

    let sent = server.received_requests().await.expect("recorded");
    assert!(
        sent[0].headers.get("anthropic-beta").is_none(),
        "Vertex rejects a beta it does not support, so none is sent undeclared"
    );
}
