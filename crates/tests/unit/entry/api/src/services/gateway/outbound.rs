//! End-to-end outbound adapter tests against wiremock'd upstreams. Drives
//! each adapter's `send` method (buffered + streaming) so we exercise the
//! per-provider request builders and response parsers without unsafe internal
//! access.

use std::collections::HashMap;

use futures_util::StreamExt;
use serde_json::json;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice, Role,
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
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new(provider),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
    }
}

fn buffered_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: Some("be helpful".into()),
        messages: vec![
            CanonicalMessage {
                role: Role::User,
                content: vec![CanonicalContent::Text("hi".into())],
            },
            CanonicalMessage {
                role: Role::Assistant,
                content: vec![CanonicalContent::Text("hello".into())],
            },
        ],
        max_tokens: 64,
        temperature: Some(0.5),
        top_p: Some(0.9),
        top_k: Some(40),
        stop_sequences: vec!["END".into()],
        tools: vec![CanonicalTool {
            name: "t".into(),
            description: Some("do".into()),
            input_schema: json!({"type":"object"}),
        }],
        tool_choice: Some(CanonicalToolChoice::Auto),
        stream: false,
        thinking: None,
        metadata: None,
    }
}

fn streaming_request() -> CanonicalRequest {
    let mut r = buffered_request();
    r.stream = true;
    r
}

#[tokio::test]
async fn anthropic_outbound_buffered_parses_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_a",
            "type": "message",
            "role": "assistant",
            "model": "upstream-1",
            "content": [{ "type": "text", "text": "hello back" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 2 }
        })))
        .mount(&server)
        .await;

    let adapter = AnthropicOutbound;
    let route = route("anthropic");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    match outcome {
        OutboundOutcome::Buffered(r) => {
            assert_eq!(r.id, "msg_a");
            assert!(matches!(r.content.first(), Some(CanonicalContent::Text(_))));
        },
        _ => panic!("expected buffered"),
    }
}

#[tokio::test]
async fn anthropic_outbound_buffered_propagates_upstream_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let adapter = AnthropicOutbound;
    let route = route("anthropic");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let res = adapter.send(ctx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn anthropic_outbound_streaming_returns_stream() {
    let server = MockServer::start().await;
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"m\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\n";
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;

    let adapter = AnthropicOutbound;
    let route = route("anthropic");
    let req = streaming_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    match outcome {
        OutboundOutcome::Streaming(mut stream) => {
            let mut count = 0;
            while let Some(_chunk) = stream.next().await {
                count += 1;
                if count > 5 {
                    break;
                }
            }
        },
        _ => panic!("expected streaming"),
    }
}

#[tokio::test]
async fn openai_chat_outbound_buffered_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-x",
            "object": "chat.completion",
            "created": 1_700_000_000_i64,
            "model": "upstream-1",
            "choices": [{
                "index": 0,
                "message": {"role":"assistant","content":"yo"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}
        })))
        .mount(&server)
        .await;
    let adapter = OpenAiChatOutbound;
    let route = route("openai");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn openai_chat_outbound_streaming_returns_stream() {
    let server = MockServer::start().await;
    let sse = "data: {\"id\":\"a\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"\
               upstream-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\ndata: \
               [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;
    let adapter = OpenAiChatOutbound;
    let route = route("openai");
    let req = streaming_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    if let OutboundOutcome::Streaming(mut stream) = outcome {
        let mut count = 0;
        while let Some(_chunk) = stream.next().await {
            count += 1;
            if count > 5 {
                break;
            }
        }
    } else {
        panic!("expected streaming");
    }
}

#[tokio::test]
async fn openai_chat_outbound_buffered_propagates_upstream_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate"))
        .mount(&server)
        .await;
    let adapter = OpenAiChatOutbound;
    let route = route("openai");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let res = adapter.send(ctx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn openai_responses_outbound_buffered_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "resp_x",
            "object": "response",
            "created_at": 1,
            "status": "completed",
            "model": "upstream-1",
            "output": [
                { "type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
                  "content": [{"type":"output_text","text":"hello","annotations":[]}]
                }
            ],
            "usage": {"input_tokens":1,"output_tokens":2,"total_tokens":3}
        })))
        .mount(&server)
        .await;
    let adapter = OpenAiResponsesOutbound;
    let route = route("openai");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn provider_tags_are_stable() {
    assert_eq!(AnthropicOutbound.provider_tag(), "anthropic");
    assert_eq!(OpenAiChatOutbound.provider_tag(), "openai");
    let _ = OpenAiResponsesOutbound.provider_tag();
}

// Ensure each adapter exercises tool/tool_choice/stop_sequences variants by
// running the buffered path with a request that touches each.
#[tokio::test]
async fn anthropic_outbound_buffered_handles_rich_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg", "type": "message", "role": "assistant",
            "model": "upstream-1",
            "content": [
                {"type":"text","text":"ok"},
                {"type":"tool_use","id":"t1","name":"do","input":{}}
            ],
            "stop_reason": "tool_use",
            "usage": { "input_tokens": 1, "output_tokens": 2 }
        })))
        .mount(&server)
        .await;
    let adapter = AnthropicOutbound;
    let route_a = route("anthropic");
    let mut req = buffered_request();
    req.tool_choice = Some(CanonicalToolChoice::Tool("do".into()));
    let ctx = OutboundCtx {
        route: &route_a,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    if let OutboundOutcome::Buffered(r) = outcome {
        assert!(
            r.content
                .iter()
                .any(|c| matches!(c, CanonicalContent::ToolUse { .. }))
        );
    } else {
        panic!("expected buffered");
    }
}

#[tokio::test]
async fn anthropic_outbound_buffered_handles_invalid_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let adapter = AnthropicOutbound;
    let route_a = route("anthropic");
    let req = buffered_request();
    let ctx = OutboundCtx {
        route: &route_a,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let res = adapter.send(ctx).await;
    assert!(res.is_err());
}

fn variants_for_tool_choice() -> Vec<CanonicalToolChoice> {
    vec![
        CanonicalToolChoice::Auto,
        CanonicalToolChoice::Any,
        CanonicalToolChoice::Required,
        CanonicalToolChoice::None,
        CanonicalToolChoice::Tool("named".into()),
    ]
}

#[tokio::test]
async fn openai_chat_outbound_buffered_covers_tool_choice_variants() {
    for tc in variants_for_tool_choice() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id":"x","object":"chat.completion","created":1,"model":"upstream-1",
                "choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}
            })))
            .mount(&server)
            .await;
        let adapter = OpenAiChatOutbound;
        let route_a = route("openai");
        let mut req = buffered_request();
        req.tool_choice = Some(tc);
        let ctx = OutboundCtx {
            route: &route_a,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
        };
        let _ = adapter.send(ctx).await.expect("ok");
    }
}

#[tokio::test]
async fn anthropic_outbound_buffered_covers_tool_choice_variants() {
    for tc in variants_for_tool_choice() {
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
        let adapter = AnthropicOutbound;
        let route_a = route("anthropic");
        let mut req = buffered_request();
        req.tool_choice = Some(tc);
        let ctx = OutboundCtx {
            route: &route_a,
            endpoint: &server.uri(),
            api_key: "k",
            request: &req,
            upstream_model: "upstream-1",
        };
        let _ = adapter.send(ctx).await.expect("ok");
    }
}

#[tokio::test]
async fn openai_chat_outbound_buffered_covers_messages_with_tools_and_images() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"x","object":"chat.completion","created":1,"model":"upstream-1",
            "choices":[{"index":0,"message":{"role":"assistant","content":null,
                "tool_calls":[{"id":"c1","type":"function","function":{"name":"f","arguments":"{}"}}]
            },"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}
        })))
        .mount(&server)
        .await;
    let adapter = OpenAiChatOutbound;
    let route_o = route("openai");
    let mut req = buffered_request();
    req.messages.push(CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Image(
            systemprompt_api::services::gateway::protocol::canonical::ImageSource::Url(
                "https://x".into(),
            ),
        )],
    });
    req.messages.push(CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id: "t1".into(),
            content: vec![CanonicalContent::Text("res".into())],
            is_error: false,
        }],
    });
    let ctx = OutboundCtx {
        route: &route_o,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
    };
    let outcome = adapter.send(ctx).await.expect("ok");
    if let OutboundOutcome::Buffered(r) = outcome {
        assert!(r.stop_reason.is_some());
        assert!(
            r.content
                .iter()
                .any(|c| matches!(c, CanonicalContent::ToolUse { .. }))
        );
    }
}
