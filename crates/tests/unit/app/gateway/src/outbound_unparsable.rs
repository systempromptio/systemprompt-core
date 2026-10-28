use super::support;

use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalReasoning, CanonicalRequest, CanonicalSampling,
    CanonicalTooling, Role,
};
use systemprompt_gateway::protocol::outbound::gemini::GeminiOutbound;
use systemprompt_gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_gateway::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use systemprompt_gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundError, UpstreamError,
};
use systemprompt_identifiers::ModelId;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn request() -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        cache_control: None,
        system: vec![],
        max_tokens: 16,
        stream: false,
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

async fn send_against<A: OutboundAdapter>(
    adapter: &A,
    provider: &str,
    route_path: &str,
    body: &str,
) -> OutboundError {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(route_path))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let hop = systemprompt_catalog_fixtures::hop(
        provider,
        systemprompt_wire::WireProtocol::Anthropic,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    );
    let req = request();
    let ctx = OutboundCtx {
        target: &hop,
        upstream: &support::api_key_call(&server.uri(), "k"),
        http: &reqwest::Client::new(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: None,
    };
    let built = adapter.build_body(&ctx).expect("body builds");
    match adapter.send(ctx, &built).await {
        Ok(_) => panic!("a malformed success body must not produce a response"),
        Err(e) => e,
    }
}

fn assert_defective_status(err: &OutboundError, needle: &str) {
    match err.upstream() {
        Some(UpstreamError::Status { message, .. }) => {
            assert!(message.contains(needle), "{message}");
        },
        other => panic!("expected an upstream status error, got {other:?}: {err}"),
    }
}

#[tokio::test]
async fn an_openai_chat_success_body_with_mistyped_fields_is_refused() {
    let err = send_against(
        &OpenAiChatOutbound,
        "openai",
        "/chat/completions",
        r#"{"choices":[{"index":"zero"}],"usage":{},"id":42}"#,
    )
    .await;
    assert_defective_status(&err, "\"id\":42");
}

#[tokio::test]
async fn an_openai_chat_success_body_that_is_not_json_is_a_decode_error() {
    let err = send_against(&OpenAiChatOutbound, "openai", "/chat/completions", "<html>").await;
    assert!(
        matches!(
            err,
            OutboundError::DecodeBody {
                wire: "openai-chat",
                ..
            }
        ),
        "{err}"
    );
}

#[tokio::test]
async fn an_openai_responses_success_body_with_mistyped_fields_is_refused() {
    let err = send_against(
        &OpenAiResponsesOutbound,
        "openai",
        "/responses",
        r#"{"output":[{"type":7}],"usage":{},"id":42}"#,
    )
    .await;
    assert_defective_status(&err, "\"id\":42");
}

#[tokio::test]
async fn a_gemini_success_body_with_mistyped_fields_is_refused() {
    let err = send_against(
        &GeminiOutbound,
        "gemini",
        "/models/upstream-1:generateContent",
        r#"{"candidates":[{"content":7}],"usageMetadata":{},"modelVersion":42}"#,
    )
    .await;
    assert_defective_status(&err, "\"modelVersion\":42");
}
