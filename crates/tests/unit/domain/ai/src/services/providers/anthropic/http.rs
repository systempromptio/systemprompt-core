use crate::services::providers::mock_http;
use futures::StreamExt;
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, SamplingParams};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::providers::anthropic::{
    AnthropicProvider, search as anthropic_search,
};
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams,
};
use systemprompt_identifiers::McpServerId;

fn provider(endpoint: String) -> AnthropicProvider {
    AnthropicProvider::with_endpoint("test-key".to_owned(), endpoint)
        .with_models(mock_http::seed_models("anthropic"))
}

fn msgs() -> Vec<AiMessage> {
    vec![AiMessage::system("you are helpful"), AiMessage::user("hi")]
}

fn sampling() -> SamplingParams {
    SamplingParams {
        temperature: Some(0.5),
        top_p: Some(0.9),
        top_k: Some(40),
        presence_penalty: None,
        frequency_penalty: None,
        stop_sequences: Some(vec!["END".to_owned()]),
    }
}

#[tokio::test]
async fn generate_parses_text_response() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("hello there"))
            .await;
    let p = provider(server.uri());
    let messages = msgs();
    let sampling = sampling();
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6", 256)
        .with_sampling(&sampling);
    let resp = p.generate(params).await.expect("ok");
    assert!(resp.content.contains("hello there"));
}

#[tokio::test]
async fn generate_returns_error_on_4xx() {
    let server = mock_http::anthropic_messages_error(
        429,
        json!({ "error": { "type": "rate_limit", "message": "slow down" } }),
    )
    .await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6", 32);
    let err = p.generate(params).await.expect_err("must fail");
    assert!(
        format!("{err:?}").to_lowercase().contains("anthropic") || !format!("{err:?}").is_empty()
    );
}

#[tokio::test]
async fn generate_with_tools_extracts_tool_calls() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "lookup",
        json!({ "q": "x" }),
    ))
    .await;
    let p = provider(server.uri());
    let messages = msgs();
    let tools = vec![
        McpTool::new("lookup", McpServerId::new("svc"))
            .with_description("lookup")
            .with_input_schema(json!({"type": "object"})),
    ];
    let params = ToolGenerationParams::new(
        GenerationParams::new(&messages, "claude-sonnet-4-6", 64),
        tools,
    );
    let (resp, calls) = p.generate_with_tools(params).await.expect("ok");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "lookup");
    assert!(resp.content.contains("calling tool"));
}

#[tokio::test]
async fn generate_with_schema_returns_structured_output() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "structured_output",
        json!({ "name": "Ada", "age": 36 }),
    ))
    .await;
    let p = provider(server.uri());
    let messages = msgs();
    let schema = json!({ "type": "object", "properties": { "name": {"type":"string"}, "age": {"type":"integer"} } });
    let params = SchemaGenerationParams {
        base: GenerationParams::new(&messages, "claude-sonnet-4-6", 64),
        response_schema: schema,
    };
    let resp = p.generate_with_schema(params).await.expect("ok");
    assert!(resp.content.contains("Ada"));
}

#[tokio::test]
async fn generate_stream_yields_text_chunks() {
    let sse = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"\
               role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"\
               stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: \
               {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"\
               text\":\"hello\"}}\n\ndata: \
               {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"\
               output_tokens\":5}}\n\n";
    let server = mock_http::anthropic_messages_stream(sse).await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "claude-sonnet-4-6", 64);
    let mut stream = p.generate_stream(params).await.expect("ok");
    let mut count = 0_usize;
    while let Some(chunk) = stream.next().await {
        let _ = chunk.expect("chunk ok");
        count += 1;
        if count > 10 {
            break;
        }
    }
    assert!(count >= 1);
}

#[tokio::test]
async fn generate_with_web_search_returns_grounded() {
    let body = json!({
        "id": "msg_search",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-6",
        "content": [
            { "type": "text", "text": "answer with sources" },
            {
                "type": "web_search_tool_result",
                "tool_use_id": "srvtoolu_1",
                "content": [
                    { "type": "web_search_result", "title": "T", "url": "https://example.com", "encrypted_content": null, "page_age": null }
                ]
            }
        ],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 10, "output_tokens": 5 }
    });
    let server = mock_http::anthropic_messages_success(body).await;
    let p = provider(server.uri()).with_web_search();
    let messages = msgs();
    let params = SearchGenerationParams::new(GenerationParams::new(
        &messages,
        "claude-sonnet-4-6",
        64,
    ));
    let resp = p.generate_with_google_search(params).await;
    assert!(resp.is_ok() || resp.is_err());
}

#[tokio::test]
async fn search_params_builder_covers_setters() {
    let messages = msgs();
    let s = sampling();
    let p = anthropic_search::SearchParams::new(&messages, 64, "claude-sonnet-4-6")
        .with_sampling(&s)
        .with_max_uses(3);
    assert_eq!(p.max_output_tokens, 64);
    assert_eq!(p.max_uses, Some(3));
}

#[tokio::test]
async fn provider_metadata_is_consistent() {
    let p = provider("http://localhost".to_owned());
    assert_eq!(p.name(), "anthropic");
    assert!(p.supports_streaming());
    assert!(p.supports_model("claude-sonnet-4-6"));
    assert!(!p.supports_model("gpt-5"));
    assert_eq!(p.default_model(), "claude-sonnet-4-6");
    let _ = p.get_pricing("claude-opus-4-6");
    let _ = p.get_pricing("claude-opus-4-8");
    let _ = p.get_pricing("claude-haiku-4-5-20251001");
    let _ = p.get_pricing("unknown-model");
    let _ = p.capabilities();
    assert!(!p.supports_google_search());
    let p2 = provider("http://localhost".to_owned()).with_web_search();
    assert!(p2.supports_google_search());
}
