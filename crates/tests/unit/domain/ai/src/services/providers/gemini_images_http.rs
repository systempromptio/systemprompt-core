use serde_json::json;
use systemprompt_ai::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageResolution,
};
use systemprompt_test_fixtures::fixture_user_id;
use systemprompt_ai::services::providers::gemini_images::GeminiImageProvider;
use systemprompt_ai::services::providers::image_provider_trait::ImageProvider;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_request(prompt: &str) -> ImageGenerationRequest {
    ImageGenerationRequest {
        prompt: prompt.to_owned(),
        model: None,
        resolution: ImageResolution::OneK,
        aspect_ratio: AspectRatio::Square,
        reference_images: vec![],
        enable_search_grounding: false,
        user_id: fixture_user_id(),
        session_id: None,
        trace_id: None,
        mcp_execution_id: None,
    }
}

#[tokio::test]
async fn generate_image_returns_inline_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        { "inlineData": { "mimeType": "image/png", "data": "CCCC" } }
                    ]
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": { "promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2 }
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let resp = p.generate_image(&make_request("hi")).await.expect("ok");
    assert_eq!(resp.image_data, "CCCC");
    assert_eq!(resp.mime_type, "image/png");
}

#[tokio::test]
async fn generate_image_rejects_long_prompt() {
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), "http://127.0.0.1:1".to_owned());
    let huge = "x".repeat(9000);
    let err = p
        .generate_image(&make_request(&huge))
        .await
        .expect_err("too long");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_unsupported_model() {
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), "http://127.0.0.1:1".to_owned());
    let mut req = make_request("ok");
    req.model = Some("nope".to_owned());
    let err = p.generate_image(&req).await.expect_err("bad model");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_handles_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(429).set_body_string("limit"))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p.generate_image(&make_request("ok")).await.expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn batch_aggregates_responses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": { "role": "model", "parts": [
                    { "inlineData": { "mimeType": "image/png", "data": "ZZ" } }
                ]},
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": { "promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2 }
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let resp = p
        .generate_batch(&[make_request("a"), make_request("b")])
        .await
        .expect("ok");
    assert_eq!(resp.len(), 2);
}

#[tokio::test]
async fn provider_metadata_is_consistent() {
    let p = GeminiImageProvider::new("k".to_owned())
        .with_default_model("gemini-2.5-flash-image".to_owned())
        .with_model_definitions(std::collections::HashMap::new());
    assert_eq!(p.name(), "gemini-image");
    assert_eq!(p.default_model(), "gemini-2.5-flash-image");
    let caps = p.capabilities();
    assert!(caps.supports_batch);
    assert!(caps.supports_search_grounding);
    assert!(p.supports_model("gemini-2.5-flash-image"));
    assert!(!p.supports_model("dall-e-3"));
    assert!(p.supports_resolution(&ImageResolution::FourK));
    assert!(p.supports_aspect_ratio(&AspectRatio::UltraWide));
}
