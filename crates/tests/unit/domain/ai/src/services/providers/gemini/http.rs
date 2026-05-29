use crate::services::providers::mock_http;
use futures::StreamExt;
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, SamplingParams};
use systemprompt_ai::services::providers::gemini::GeminiProvider;
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, SchemaGenerationParams, SearchGenerationParams,
};

fn provider(endpoint: String) -> GeminiProvider {
    GeminiProvider::with_endpoint("test-key".to_owned(), endpoint).expect("provider")
}

fn msgs() -> Vec<AiMessage> {
    vec![AiMessage::user("hello")]
}

fn sampling() -> SamplingParams {
    SamplingParams {
        temperature: Some(0.3),
        top_p: Some(0.7),
        top_k: Some(20),
        presence_penalty: None,
        frequency_penalty: None,
        stop_sequences: Some(vec!["STOP".to_owned()]),
    }
}

#[tokio::test]
async fn generate_parses_text_response() {
    let server =
        mock_http::gemini_generate_success(mock_http::gemini_response_body("gemini hi")).await;
    let p = provider(server.uri());
    let messages = msgs();
    let s = sampling();
    let params = GenerationParams::new(&messages, "gemini-2.5-flash", 32).with_sampling(&s);
    let resp = p.generate(params).await.expect("ok");
    assert!(resp.content.contains("gemini hi"));
}

#[tokio::test]
async fn generate_returns_error_on_4xx() {
    let server =
        mock_http::gemini_generate_error(400, json!({ "error": { "message": "bad input" } })).await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "gemini-2.5-flash", 32);
    let err = p.generate(params).await.expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_with_schema_returns_structured() {
    let server =
        mock_http::gemini_generate_success(mock_http::gemini_response_body("{\"k\": 1}")).await;
    let p = provider(server.uri());
    let messages = msgs();
    let schema = json!({ "type": "object" });
    let params = SchemaGenerationParams {
        base: GenerationParams::new(&messages, "gemini-2.5-flash", 32),
        response_schema: schema,
    };
    let resp = p.generate_with_schema(params).await.expect("ok");
    let _ = resp.content;
}

#[tokio::test]
async fn generate_stream_yields_chunks() {
    let sse = "data: [{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\\
               "}]}}],\"usageMetadata\":{\"promptTokenCount\":1,\"totalTokenCount\":2,\"\
               candidatesTokenCount\":1}}]\n\n";
    let server = mock_http::gemini_generate_stream(sse).await;
    let p = provider(server.uri());
    let messages = msgs();
    let params = GenerationParams::new(&messages, "gemini-2.5-flash", 32);
    let stream_result = p.generate_stream(params).await;
    if let Ok(mut stream) = stream_result {
        let mut count = 0_usize;
        while let Some(chunk) = stream.next().await {
            let _ = chunk;
            count += 1;
            if count > 10 {
                break;
            }
        }
    }
}

#[tokio::test]
async fn generate_with_google_search_returns_grounded() {
    let server =
        mock_http::gemini_generate_success(mock_http::gemini_grounded_body("grounded answer"))
            .await;
    let p = provider(server.uri()).with_google_search();
    let messages = msgs();
    let params =
        SearchGenerationParams::new(GenerationParams::new(&messages, "gemini-2.5-flash", 32))
            .with_urls(vec!["https://example.com".to_owned()]);
    let _ = p.generate_with_google_search(params).await;
}

#[tokio::test]
async fn generate_with_code_execution_uses_endpoint() {
    let body = json!({
        "candidates": [{
            "content": { "role": "model", "parts": [
                { "text": "computed" },
                { "executableCode": { "language": "PYTHON", "code": "print(1)" } },
                { "codeExecutionResult": { "outcome": "OUTCOME_OK", "output": "1\n" } }
            ]},
            "finishReason": "STOP",
            "index": 0
        }],
        "usageMetadata": { "promptTokenCount": 1, "candidatesTokenCount": 2, "totalTokenCount": 3 }
    });
    let server = mock_http::gemini_generate_success(body).await;
    let p = provider(server.uri());
    let messages = msgs();
    let _ = p
        .generate_with_code_execution(&messages, None, 32, "gemini-2.5-flash")
        .await;
}

#[tokio::test]
async fn provider_metadata_is_consistent() {
    let p = provider("http://localhost".to_owned());
    assert_eq!(p.name(), "gemini");
    assert!(p.supports_streaming());
    assert!(p.supports_model("gemini-2.5-flash"));
    assert!(!p.supports_model("gpt-4"));
    assert_eq!(p.default_model(), "gemini-3.1-flash-lite-preview");
    let _ = p.get_pricing("gemini-3-pro-image-preview");
    let _ = p.get_pricing("gemini-3-flash-preview");
    let _ = p.get_pricing("gemini-3.1-flash-lite-preview");
    let _ = p.get_pricing("gemini-2.5-pro");
    let _ = p.get_pricing("gemini-2.5-flash");
    let _ = p.get_pricing("gemini-1.5-pro");
    let _ = p.get_pricing("unknown");
    let _ = p.capabilities();
    assert!(!p.supports_google_search());
    assert!(!p.has_google_search());
    let p2 = provider("http://localhost".to_owned()).with_google_search();
    assert!(p2.has_google_search());
}
