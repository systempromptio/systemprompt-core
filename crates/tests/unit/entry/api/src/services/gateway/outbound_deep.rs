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
use systemprompt_api::services::gateway::protocol::outbound::gemini::GeminiOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

// `OutboundOutcome` is not `Debug` (it carries a boxed stream), so the failure
// cases cannot use `expect_err`.
fn expect_failure(outcome: anyhow::Result<OutboundOutcome>) -> anyhow::Error {
    match outcome {
        Ok(_) => panic!("this request must not produce a usable response"),
        Err(e) => e,
    }
}

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
        when: None,
        requires: None,
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
                        detail: None,
                    }),
                    CanonicalContent::Image(ImageSource::Url {
                        url: "https://x/y".into(),
                        detail: None,
                    }),
                ],
            },
            CanonicalMessage {
                role: Role::Assistant,
                content: vec![
                    CanonicalContent::Thinking {
                        id: None,
                        encrypted_content: None,
                        text: "let me think".into(),
                        signature: Some("sig".into()),
                    },
                    CanonicalContent::Text("here's my answer".into()),
                    CanonicalContent::ToolUse {
                        id: "tu1".into(),
                        name: "search".into(),
                        input: json!({"q": "rust"}),
                        signature: None,
                    },
                ],
            },
            CanonicalMessage {
                role: Role::Tool,
                content: vec![CanonicalContent::ToolResult {
                    tool_use_id: "tu1".into(),
                    content: vec![CanonicalContent::Text("results".into())],
                    is_error: false,
                    structured_content: None,
                    meta: None,
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
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let outcome = send_via(&AnthropicOutbound, ctx).await.expect("ok");
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let outcome = send_via(&OpenAiChatOutbound, ctx).await.expect("ok");
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let outcome = send_via(&OpenAiResponsesOutbound, ctx).await.expect("ok");
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let res = send_via(&OpenAiResponsesOutbound, ctx).await;
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let res = send_via(&OpenAiResponsesOutbound, ctx).await;
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
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    };
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let outcome = send_via(&AnthropicOutbound, ctx).await.expect("ok");
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
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let outcome = send_via(&OpenAiChatOutbound, ctx).await.expect("ok");
    if let OutboundOutcome::Streaming(_s) = outcome {
        // Stream returned — that's the branch we want to cover.
    } else {
        panic!("expected streaming outcome");
    }
}

#[tokio::test]
async fn gemini_outbound_with_rich_request_buffered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/upstream-1:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": "answer"}]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 4}
        })))
        .mount(&server)
        .await;
    let r = route("gemini");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    let outcome = send_via(&GeminiOutbound, ctx).await.expect("ok");

    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));
}

#[tokio::test]
async fn gemini_outbound_sends_the_api_key_as_a_header_not_a_query_parameter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/upstream-1:generateContent"))
        .and(header("x-goog-api-key", "secret-key"))
        .and(header("x-custom", "value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {"role": "model", "parts": [{"text": "ok"}]},
                "finishReason": "STOP"
            }]
        })))
        .mount(&server)
        .await;
    let r = route("gemini");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "secret-key",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    // The mock only matches when both the credential header and the route's
    // extra headers are present, so a match is the assertion.
    send_via(&GeminiOutbound, ctx)
        .await
        .expect("the request must carry the api key header and the route extras");
}

#[tokio::test]
async fn gemini_outbound_streams_from_the_sse_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/upstream-1:streamGenerateContent"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(
                    "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\
                     \"hi\"}]}}]}\n\n",
                ),
        )
        .mount(&server)
        .await;
    let r = route("gemini");
    let mut req = rich_request();
    req.stream = true;
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    let outcome = send_via(&GeminiOutbound, ctx).await.expect("ok");

    assert!(
        matches!(outcome, OutboundOutcome::Streaming(_)),
        "a streaming request must select the SSE upstream path"
    );
}

#[tokio::test]
async fn a_gemini_upstream_rejection_is_reported_as_an_upstream_status_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/upstream-1:generateContent"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"code": 429, "message": "quota exceeded"}
        })))
        .mount(&server)
        .await;
    let r = route("gemini");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    let err = expect_failure(send_via(&GeminiOutbound, ctx).await);

    let upstream = err
        .downcast_ref::<UpstreamError>()
        .expect("the failure must stay an UpstreamError so the gateway can relay it");
    let UpstreamError::Status { status, .. } = upstream else {
        panic!("a rejected request must carry the upstream status");
    };
    assert_eq!(*status, 429);
}

#[tokio::test]
async fn an_unreachable_gemini_endpoint_is_a_transport_error() {
    let r = route("gemini");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: "http://127.0.0.1:1",
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    let err = expect_failure(send_via(&GeminiOutbound, ctx).await);

    assert!(
        matches!(
            err.downcast_ref::<UpstreamError>(),
            Some(UpstreamError::Transport { .. })
        ),
        "a connection failure must be distinguishable from an upstream rejection: {err}"
    );
}

#[tokio::test]
async fn a_gemini_response_that_is_not_json_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/upstream-1:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>not json</html>"))
        .mount(&server)
        .await;
    let r = route("gemini");
    let req = rich_request();
    let ctx = OutboundCtx {
        route: &r,
        endpoint: &server.uri(),
        api_key: "k",
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };

    let err = expect_failure(send_via(&GeminiOutbound, ctx).await);

    assert!(err.to_string().contains("not valid JSON"), "{err}");
}
