// Tool-use and tool-result drivers for the Gemini provider, exercised through
// the public `AiProvider` trait against a wiremock endpoint. These cover the
// schema-transform / name-mapper round trip in `gemini/tools.rs` and
// `gemini/tool_conversion.rs`: convert_tools, resolve_response (function-call
// name resolution), and the tool-result turn assembly.

use crate::services::providers::mock_http;
use serde_json::json;
use systemprompt_ai::MessageRole;
use systemprompt_ai::models::ai::AiMessage;
use systemprompt_ai::models::tools::{CallToolResult, McpTool, ToolCall};
use systemprompt_ai::services::providers::gemini::GeminiProvider;
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, ToolGenerationParams, ToolResultsParams,
};
use systemprompt_identifiers::{AiToolCallId, McpServerId};

fn provider(endpoint: String) -> GeminiProvider {
    GeminiProvider::with_endpoint("test-key".to_owned(), endpoint)
        .expect("provider")
        .with_models(mock_http::seed_models("gemini"))
}

fn msgs() -> Vec<AiMessage> {
    vec![AiMessage {
        role: MessageRole::User,
        content: "what is the weather".to_owned(),
        parts: Vec::new(),
    }]
}

fn weather_tool() -> McpTool {
    McpTool {
        name: "get_weather".to_owned(),
        description: Some("Look up the weather for a city".to_owned()),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        })),
        output_schema: None,
        service_id: McpServerId::new("weather-service"),
        terminal_on_success: false,
        model_config: None,
    }
}

fn gemini_function_call_body(name: &str, args: serde_json::Value) -> serde_json::Value {
    json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": [{ "functionCall": { "name": name, "args": args } }]
            },
            "finishReason": "STOP",
            "index": 0,
            "safetyRatings": []
        }],
        "usageMetadata": {
            "promptTokenCount": 7,
            "candidatesTokenCount": 5,
            "totalTokenCount": 12
        }
    })
}

#[tokio::test]
async fn generate_with_tools_returns_function_call() {
    let body = gemini_function_call_body("get_weather", json!({ "city": "Paris" }));
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();
    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolGenerationParams::new(base, vec![weather_tool()]);

    let (response, tool_calls) = p.generate_with_tools(params).await.expect("ok");

    assert_eq!(response.provider, "gemini");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(tool_calls[0].arguments["city"], json!("Paris"));
    assert_eq!(response.tool_calls.len(), 1);
}

#[tokio::test]
async fn generate_with_tools_text_response_has_no_tool_calls() {
    let body = mock_http::gemini_response_body("It is sunny today.");
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();
    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolGenerationParams::new(base, vec![weather_tool()]);

    let (response, tool_calls) = p.generate_with_tools(params).await.expect("ok");

    assert!(tool_calls.is_empty());
    assert!(response.content.contains("sunny"));
}

#[tokio::test]
async fn generate_with_tools_empty_tools_still_succeeds() {
    let body = mock_http::gemini_response_body("No tools available.");
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();
    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolGenerationParams::new(base, Vec::new());

    let (response, tool_calls) = p.generate_with_tools(params).await.expect("ok");

    assert!(tool_calls.is_empty());
    assert!(response.content.contains("No tools"));
}

#[tokio::test]
async fn generate_with_tools_dedupes_identical_tool_names() {
    let body = gemini_function_call_body("get_weather", json!({ "city": "Berlin" }));
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();
    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolGenerationParams::new(base, vec![weather_tool(), weather_tool()]);

    let (_response, tool_calls) = p.generate_with_tools(params).await.expect("ok");

    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].arguments["city"], json!("Berlin"));
}

#[tokio::test]
async fn generate_with_tools_propagates_http_error() {
    let server =
        mock_http::gemini_generate_error(429, json!({ "error": { "message": "rate limited" } }))
            .await;
    let p = provider(server.uri());
    let messages = msgs();
    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolGenerationParams::new(base, vec![weather_tool()]);

    let err = p.generate_with_tools(params).await.expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_with_tool_results_synthesizes_reply() {
    let body = mock_http::gemini_response_body("The weather in Paris is sunny.");
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();

    let tool_calls = vec![ToolCall {
        ai_tool_call_id: AiToolCallId::new("call-1"),
        name: "get_weather".to_owned(),
        arguments: json!({ "city": "Paris" }),
    }];
    let tool_results = vec![CallToolResult::success(vec![rmcp::model::ContentBlock::text(
        "sunny, 24C",
    )])];

    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolResultsParams::new(base, &tool_calls, &tool_results);

    let response = p.generate_with_tool_results(params).await.expect("ok");

    assert_eq!(response.provider, "gemini");
    assert!(response.content.contains("Paris"));
}

#[tokio::test]
async fn generate_with_tool_results_handles_error_results() {
    let body = mock_http::gemini_response_body("The lookup failed, please retry.");
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();

    let tool_calls = vec![ToolCall {
        ai_tool_call_id: AiToolCallId::new("call-err"),
        name: "get_weather".to_owned(),
        arguments: json!({ "city": "Nowhere" }),
    }];
    let tool_results = vec![CallToolResult::error(vec![rmcp::model::ContentBlock::text(
        "city not found",
    )])];

    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolResultsParams::new(base, &tool_calls, &tool_results);

    let response = p.generate_with_tool_results(params).await.expect("ok");

    assert!(response.content.contains("failed"));
}

#[tokio::test]
async fn generate_with_tool_results_empty_calls_still_renders() {
    let body = mock_http::gemini_response_body("Nothing to report.");
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();

    let tool_calls: Vec<ToolCall> = Vec::new();
    let tool_results: Vec<CallToolResult> = Vec::new();

    let base = GenerationParams::new(&messages, "gemini-2.5-flash", 64);
    let params = ToolResultsParams::new(base, &tool_calls, &tool_results);

    let response = p.generate_with_tool_results(params).await.expect("ok");

    assert!(response.content.contains("Nothing"));
}
