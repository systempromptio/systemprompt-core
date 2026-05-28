//! Deeper coverage for outbound adapters — exercises rich request bodies
//! (images, tool calls, thinking, system messages, stop sequences) across all
//! three adapters so the per-provider request builders see every branch.

use std::collections::HashMap;

use serde_json::json;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::profile::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn route(provider: &str) -> GatewayRoute {
    let mut extra = HashMap::new();
    extra.insert("x-custom".to_owned(), "value".to_owned());
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new(provider),
        upstream_model: Some("upstream-1".into()),
        extra_headers: extra,
        pricing: None,
    }
}

fn rich_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: Some("be helpful".into()),
        messages: vec![
            CanonicalMessage {
                role: Role::System,
                content: vec![CanonicalContent::Text("system note".into())],
            },
            CanonicalMessage {
                role: Role::User,
                content: vec![
                    CanonicalContent::Text("look at this".into()),
                    CanonicalContent::Image(ImageSource::Base64 {
                        media_type: "image/png".into(),
                        data: "AAAA".into(),
                    }),
                    CanonicalContent::Image(ImageSource::Url("https://x/y".into())),
                ],
            },
            CanonicalMessage {
                role: Role::Assistant,
                content: vec![
                    CanonicalContent::Thinking {
                        text: "let me think".into(),
                        signature: Some("sig".into()),
                    },
                    CanonicalContent::Text("here's my answer".into()),
                    CanonicalContent::ToolUse {
                        id: "tu1".into(),
                        name: "search".into(),
                        input: json!({"q": "rust"}),
                    },
                ],
            },
            CanonicalMessage {
                role: Role::Tool,
                content: vec![CanonicalContent::ToolResult {
                    tool_use_id: "tu1".into(),
                    content: vec![CanonicalContent::Text("results".into())],
                    is_error: false,
                }],
            },
        ],
        max_tokens: 100,
        temperature: Some(0.5),
        top_p: Some(0.9),
        top_k: Some(40),
        stop_sequences: vec!["END".into(), "STOP".into()],
        tools: vec![CanonicalTool {
            name: "search".into(),
            description: Some("web search".into()),
            input_schema: json!({"type": "object"}),
        }],
        tool_choice: Some(CanonicalToolChoice::Tool("search".into())),
        stream: false,
        thinking: Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(2048),
        }),
        metadata: Some(json!({"trace": "abc"})),
    }
}

#[tokio::test]
async fn anthropic_outbound_with_rich_request_and_extra_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "m1", "type": "message", "role": "assistant",
            "model": "upstream-1",
            "content": [{"type":"text","text":"ok"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 2}
        })))
        .mount(&server)
        .await;
    let r = route("anthropic");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = AnthropicOutbound.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn openai_chat_outbound_with_rich_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "x", "object": "chat.completion", "created": 1,
            "model": "upstream-1",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "answer"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 7, "total_tokens": 12}
        })))
        .mount(&server)
        .await;
    let r = route("openai");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = OpenAiChatOutbound.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn openai_responses_outbound_with_rich_request_buffered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "resp_1",
            "object": "response",
            "model": "upstream-1",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "ok"}]
            }],
            "usage": {"input_tokens": 10, "output_tokens": 5}
        })))
        .mount(&server)
        .await;
    let r = route("openai");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = OpenAiResponsesOutbound.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn openai_responses_outbound_propagates_upstream_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal"))
        .mount(&server)
        .await;
    let r = route("openai");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let res = OpenAiResponsesOutbound.send(ctx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn openai_responses_outbound_handles_invalid_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_string("nope"))
        .mount(&server)
        .await;
    let r = route("openai");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let res = OpenAiResponsesOutbound.send(ctx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn anthropic_outbound_no_system_no_tools() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"m","type":"message","role":"assistant","model":"upstream-1",
            "content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn",
            "usage":{"input_tokens":1,"output_tokens":2}
        })))
        .mount(&server)
        .await;
    let r = route("anthropic");
    let req = CanonicalRequest {
        model: "m".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".into())],
        }],
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
    };
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = AnthropicOutbound.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn openai_chat_outbound_streaming_with_extra_headers() {
    let server = MockServer::start().await;
    let body = "data: {\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"upstream-1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"hi\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let r = route("openai");
    let mut req = rich_request();
    req.stream = true;
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = OpenAiChatOutbound.send(ctx).await.expect("ok");
    if let OutboundOutcome::Streaming(_s) = outcome {
        // Stream returned — that's the branch we want to cover.
    } else {
        panic!("expected streaming outcome");
    }
}
