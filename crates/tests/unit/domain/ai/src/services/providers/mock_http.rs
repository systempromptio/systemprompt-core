//! Wiremock-driven HTTP harness shared by the anthropic/openai/gemini
//! provider drivers. Each helper spins up a fresh `MockServer`, registers a
//! canned response, and yields the base endpoint; `api_key_target` resolves
//! a one-endpoint catalog pointed at it, the same way the in-process service
//! resolves a hop, so the provider is constructed via `with_target`.

use serde_json::json;
use systemprompt_ai::services::providers::{ServedModel, served_models_on};
use systemprompt_ai::{EndpointBinding, UpstreamTarget};
use systemprompt_manifest::services::catalog::{
    CatalogContext, DefaultCatalog, EndpointConfig, EndpointRef, ProviderConfig, ResolvedCatalog,
};
use systemprompt_test_fixtures::catalog::{CatalogFixture, model, platform, priced};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub const FIXTURE_MODEL: &str = "fixture-model";

pub fn default_catalog() -> ResolvedCatalog {
    DefaultCatalog::document()
        .expect("embedded default catalog parses")
        .resolve(&CatalogContext::default())
        .expect("embedded default catalog resolves")
}

pub fn seed_models(provider: &str) -> Vec<ServedModel> {
    let models = served_models_on(&default_catalog(), &EndpointRef::new(provider, "api"));
    assert!(
        !models.is_empty(),
        "provider '{provider}' present in default catalog"
    );
    models
}

pub fn single_endpoint_catalog(name: &str, wire: WireProtocol, url: &str) -> ResolvedCatalog {
    CatalogFixture::new()
        .credential(name, platform(name))
        .provider(name, ProviderConfig::new(name))
        .endpoint(name, "api", EndpointConfig::new(wire, url))
        .model(FIXTURE_MODEL, model(&[], 200_000, priced(1.0, 2.0, 0.1)))
        .deploy(FIXTURE_MODEL, [format!("{name}/api").as_str()])
        .build()
}

pub fn api_key_target(name: &str, wire: WireProtocol, endpoint: &str, key: &str) -> UpstreamTarget {
    let catalog = single_endpoint_catalog(name, wire, endpoint);
    let hop = &catalog.deployments(FIXTURE_MODEL)[0];
    UpstreamTarget::resolve(&Default::default(), &catalog, hop, key)
        .expect("an API key resolves against a plain endpoint")
}

pub fn seed_target(provider: &str, key: &str) -> UpstreamTarget {
    let catalog = default_catalog();
    let binding = EndpointBinding::of_endpoint(&catalog, &EndpointRef::new(provider, "api"))
        .unwrap_or_else(|e| panic!("provider '{provider}' present in default catalog: {e}"));
    let cache_key =
        systemprompt_security::credential::TokenCacheKey::platform(&binding.credential.id);
    UpstreamTarget::bind(&Default::default(), binding, key, cache_key)
        .expect("an API key resolves against the catalog endpoint")
}

pub async fn anthropic_messages_success(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn anthropic_messages_error(status: u16, body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn anthropic_messages_stream(sse_body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body.to_owned()),
        )
        .mount(&server)
        .await;
    server
}

pub async fn openai_chat_success(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn openai_chat_error(status: u16, body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn openai_chat_stream(sse_body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body.to_owned()),
        )
        .mount(&server)
        .await;
    server
}

pub async fn openai_responses_success(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn gemini_generate_success(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn gemini_generate_error(status: u16, body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:.+"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

pub async fn gemini_generate_stream(sse_body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:streamGenerateContent.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body.to_owned()),
        )
        .mount(&server)
        .await;
    server
}

pub fn anthropic_response_body(text: &str) -> serde_json::Value {
    json!({
        "id": "msg_test_01",
        "type": "message",
        "role": "assistant",
        "content": [ { "type": "text", "text": text } ],
        "model": "claude-sonnet-5",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": { "input_tokens": 10, "output_tokens": 20 }
    })
}

pub fn anthropic_tool_use_body(tool_name: &str, input: serde_json::Value) -> serde_json::Value {
    json!({
        "id": "msg_test_02",
        "type": "message",
        "role": "assistant",
        "content": [
            { "type": "text", "text": "calling tool" },
            { "type": "tool_use", "id": "toolu_01", "name": tool_name, "input": input }
        ],
        "model": "claude-sonnet-5",
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": { "input_tokens": 12, "output_tokens": 25 }
    })
}

pub fn openai_response_body(content: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1_700_000_000_i64,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 7, "total_tokens": 12 }
    })
}

pub fn openai_tool_call_body(name: &str, args: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test-tools",
        "object": "chat.completion",
        "created": 1_700_000_000_i64,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_01",
                    "type": "function",
                    "function": { "name": name, "arguments": args }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 8, "completion_tokens": 4, "total_tokens": 12 }
    })
}

pub fn gemini_response_body(text: &str) -> serde_json::Value {
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": text }] },
            "finishReason": "STOP",
            "index": 0,
            "safetyRatings": []
        }],
        "usageMetadata": {
            "promptTokenCount": 11,
            "candidatesTokenCount": 9,
            "totalTokenCount": 20
        }
    })
}

pub fn gemini_grounded_body(text: &str) -> serde_json::Value {
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": text }] },
            "finishReason": "STOP",
            "index": 0,
            "safetyRatings": [],
            "groundingMetadata": {
                "groundingChunks": [
                    { "web": { "uri": "https://example.com/a", "title": "Example A" } }
                ],
                "groundingSupports": [],
                "webSearchQueries": ["test query"]
            }
        }],
        "usageMetadata": {
            "promptTokenCount": 5,
            "candidatesTokenCount": 5,
            "totalTokenCount": 10
        }
    })
}
