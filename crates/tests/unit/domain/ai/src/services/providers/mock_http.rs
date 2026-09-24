//! Wiremock-driven HTTP harness shared by the anthropic/openai/gemini
//! provider drivers. Each helper spins up a fresh `MockServer`, registers a
//! canned response, and yields the base endpoint; `api_key_target` resolves
//! a catalog entry pointed at it, the same way the factory resolves one, so
//! the provider is constructed via `with_target`.

use std::collections::HashMap;

use serde_json::json;
use systemprompt_ai::UpstreamTarget;
use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_models::services::{
    ModelGovernance, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub fn seed_models(provider: &str) -> Vec<ProviderModel> {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in default catalog"))
        .models
        .clone()
}

pub fn api_key_target(name: &str, wire: WireProtocol, endpoint: &str, key: &str) -> UpstreamTarget {
    let entry = ProviderEntry {
        name: ProviderId::new(name),
        display_name: None,
        description: None,
        wire,
        surface: wire.surface(),
        endpoint: endpoint.to_owned(),
        api_key_secret: SecretName::new(name),
        extra_headers: HashMap::new(),
        accepted_betas: None,
        models: Vec::new(),
        governance: ModelGovernance::default(),
    };
    UpstreamTarget::resolve(&entry, key).expect("an API key resolves against a plain endpoint")
}

pub fn seed_target(provider: &str, key: &str) -> UpstreamTarget {
    let registry = ProviderRegistry::default_seed().expect("embedded default catalog parses");
    let entry = registry
        .find_provider(provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in default catalog"));
    UpstreamTarget::resolve(entry, key).expect("an API key resolves against the catalog endpoint")
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
