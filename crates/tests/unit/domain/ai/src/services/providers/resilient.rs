use std::sync::Arc;

use crate::services::providers::mock_http;
use systemprompt_ai::models::ai::{AiMessage, ResponseFormat};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_ai::services::providers::resilient_provider::ResilientProvider;
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, SchemaGenerationParams, StructuredGenerationParams,
    ToolGenerationParams, ToolResultsParams,
};
use systemprompt_identifiers::McpServerId;
use systemprompt_models::services::ResilienceSettings;

fn settings() -> ResilienceSettings {
    ResilienceSettings::default()
}

#[tokio::test]
async fn delegates_generate_to_inner() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("ok via guard"))
            .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let resilient: Arc<dyn AiProvider> =
        Arc::new(ResilientProvider::new("anthropic", Arc::new(inner), &s));

    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 32);
    let resp = resilient.generate(params).await.expect("ok");
    assert!(resp.content.contains("ok via guard"));
}

#[tokio::test]
async fn delegates_metadata() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    assert_eq!(r.name(), "anthropic");
    assert!(r.supports_streaming());
    assert!(r.supports_model("claude-sonnet-4-6-20250610"));
    assert!(!r.supports_model("nope"));
    assert_eq!(r.default_model(), "claude-sonnet-4-6-20250610");
    let _ = r.get_pricing("claude-sonnet-4-6-20250610");
    let _ = r.capabilities();
    let _ = r.supports_json_mode();
    let _ = r.supports_structured_output();
    let _ = r.supports_google_search();
    let _ = r.supports_sampling(None);
    let _ = r.as_any();
}

#[tokio::test]
async fn maps_inner_error() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 16);
    let res = r.generate(params).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn delegates_generate_with_tools() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "f",
        serde_json::json!({}),
    ))
    .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let tools = vec![McpTool::new("f", McpServerId::new("svc"))];
    let params = ToolGenerationParams::new(
        GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 16),
        tools,
    );
    let res = r.generate_with_tools(params).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn delegates_generate_with_schema() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "structured_output",
        serde_json::json!({"answer": 42}),
    ))
    .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 32);
    let params = SchemaGenerationParams::new(
        base,
        serde_json::json!({"type": "object", "properties": {"answer": {"type": "number"}}}),
    );
    let res = r.generate_with_schema(params).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn delegates_generate_structured() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("plain")).await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 32);
    let fmt = ResponseFormat::json_object();
    let params = StructuredGenerationParams::new(base, &fmt);
    let res = r.generate_structured(params).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn delegates_generate_with_tool_results() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("after-tool"))
            .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 32);
    let calls: Vec<systemprompt_ai::models::tools::ToolCall> = Vec::new();
    let results: Vec<systemprompt_ai::models::tools::CallToolResult> = Vec::new();
    let params = ToolResultsParams::new(base, &calls, &results);
    let res = r.generate_with_tool_results(params).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn delegates_generate_with_tools_stream() {
    let sse = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"\
               text_delta\",\"text\":\"hi\"}}\n\n";
    let server = mock_http::anthropic_messages_stream(sse).await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 16);
    let tools = vec![McpTool::new("f", McpServerId::new("svc"))];
    let params = ToolGenerationParams::new(base, tools);
    let stream_res = r.generate_with_tools_stream(params).await;
    assert!(stream_res.is_ok());
}

#[tokio::test]
async fn stream_open_failure_releases_permit() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 16);
    let res = r.generate_stream(params).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn stream_call_guards_path() {
    let sse = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"\
               text_delta\",\"text\":\"hi\"}}\n\n";
    let server = mock_http::anthropic_messages_stream(sse).await;
    let inner = AnthropicProvider::with_endpoint("k".to_owned(), server.uri());
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6-20250610", 16);
    let stream_res = r.generate_stream(params).await;
    assert!(stream_res.is_ok());
}
