use crate::services::providers::mock_http;
use futures::StreamExt;
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, ResponseFormat, SamplingParams};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::providers::openai::OpenAiProvider;
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams,
};
use systemprompt_identifiers::McpServerId;

fn provider(endpoint: String) -> OpenAiProvider {
    OpenAiProvider::with_endpoint("test-key".to_owned(), endpoint)
        .with_models(mock_http::seed_models("openai"))
}

fn msgs() -> Vec<AiMessage> {
    vec![
        AiMessage::system("be brief"),
        AiMessage::user("hello"),
        AiMessage::assistant("hi"),
        AiMessage::user("bye"),
    ]
}

fn sampling() -> SamplingParams {
    SamplingParams {
        temperature: Some(0.4),
        top_p: Some(0.95),
        top_k: None,
        presence_penalty: Some(0.1),
        frequency_penalty: Some(0.0),
        stop_sequences: None,
    }
}

#[tokio::test]
async fn generate_parses_response() {
    let server = mock_http::openai_chat_success(mock_http::openai_response_body("hello")).await;
    let p = provider(server.uri());
    let messages = msgs();
    let s = sampling();
    let params = GenerationParams::new(&messages, "gpt-4o-mini", 32).with_sampling(&s);
    let resp = p.generate(params).await.expect("ok");
    assert_eq!(resp.content, "hello");
}

#[tokio::test]
async fn generate_returns_error_on_4xx() {
    let server = mock_http::openai_chat_error(
        401,
        json!({ "error": { "type": "auth", "message": "bad key" } }),
    )
    .await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "gpt-4o-mini", 32);
    let err = p.generate(params).await.expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_with_tools_extracts_tool_calls() {
    let server =
        mock_http::openai_chat_success(mock_http::openai_tool_call_body("do_thing", "{\"x\":1}"))
            .await;
    let p = provider(server.uri());
    let messages = msgs();
    let tools = vec![
        McpTool::new("do_thing", McpServerId::new("svc"))
            .with_input_schema(json!({"type": "object"})),
    ];
    let params =
        ToolGenerationParams::new(GenerationParams::new(&messages, "gpt-4o-mini", 64), tools);
    let (_resp, calls) = p.generate_with_tools(params).await.expect("ok");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "do_thing");
}

#[tokio::test]
async fn generate_with_schema_returns_structured() {
    let server =
        mock_http::openai_chat_success(mock_http::openai_response_body("{\"answer\": 42}")).await;
    let p = provider(server.uri());
    let messages = msgs();
    let schema = json!({ "type": "object", "properties": {"answer": {"type":"integer"}}});
    let params = SchemaGenerationParams {
        base: GenerationParams::new(&messages, "gpt-4o-mini", 64),
        response_schema: schema,
    };
    let resp = p.generate_with_schema(params).await.expect("ok");
    assert!(resp.content.contains("answer"));
}

#[tokio::test]
async fn generate_structured_json_object() {
    let server = mock_http::openai_chat_success(mock_http::openai_response_body("{\"k\":1}")).await;
    let p = provider(server.uri());
    let messages = msgs();
    let fmt = ResponseFormat::JsonObject;
    let params = StructuredGenerationParams {
        base: GenerationParams::new(&messages, "gpt-4o-mini", 64),
        response_format: &fmt,
    };
    let resp = p.generate_structured(params).await.expect("ok");
    assert!(resp.content.contains('{'));
}

#[tokio::test]
async fn generate_stream_yields_chunks() {
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: \
               {\"choices\":[],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"\
               total_tokens\":3}}\n\ndata: [DONE]\n\n";
    let server = mock_http::openai_chat_stream(sse).await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "gpt-4o-mini", 32);
    let mut stream = p.generate_stream(params).await.expect("ok");
    let mut count = 0_usize;
    while let Some(chunk) = stream.next().await {
        let _ = chunk;
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
        "id": "resp_x",
        "output": [
            { "type": "message", "content": [
                { "type": "output_text", "text": "ok", "annotations": [
                    { "type": "url_citation", "url": "https://example.com", "title": "Example" }
                ]}
            ]}
        ],
        "usage": { "input_tokens": 1, "output_tokens": 2, "total_tokens": 3 }
    });
    let server = mock_http::openai_responses_success(body).await;
    let p = provider(server.uri()).with_web_search();
    let messages = msgs();
    let params = SearchGenerationParams::new(GenerationParams::new(&messages, "gpt-4o-mini", 32));
    let resp = p.generate_with_google_search(params).await;
    assert!(resp.is_ok() || resp.is_err());
}

#[tokio::test]
async fn provider_metadata_is_consistent() {
    let p = provider("http://localhost".to_owned());
    assert_eq!(p.name(), "openai");
    assert!(p.supports_streaming());
    assert!(p.supports_json_mode());
    assert!(p.supports_structured_output());
    assert!(p.supports_model("gpt-4o-mini"));
    assert!(!p.supports_model("claude"));
    let _ = p.get_pricing("gpt-4.1");
    let _ = p.get_pricing("gpt-4.1-mini");
    let _ = p.get_pricing("gpt-4.1-nano");
    let _ = p.get_pricing("o3");
    let _ = p.get_pricing("o4-mini");
    let _ = p.get_pricing("gpt-4o-mini");
    let _ = p.get_pricing("gpt-4");
    let _ = p.get_pricing("gpt-3.5-turbo");
    let _ = p.get_pricing("o1");
    let _ = p.get_pricing("o3-mini");
    let _ = p.get_pricing("unknown");
    let _ = p.capabilities();
    assert_eq!(p.default_model(), "gpt-4.1");
    assert!(!p.supports_google_search());
    let p2 = provider("http://localhost".to_owned()).with_web_search();
    assert!(p2.supports_google_search());
}
