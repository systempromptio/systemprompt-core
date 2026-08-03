use serde_json::json;
use systemprompt_ai::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageResolution,
};
use systemprompt_ai::services::providers::gemini_images::GeminiImageProvider;
use systemprompt_ai::services::providers::image_provider_trait::ImageProvider;
use systemprompt_models::services::{ModelCapabilities, ModelDefinition};
use systemprompt_test_fixtures::fixture_user_id;
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
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("err");
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
async fn generate_image_rejects_empty_candidates() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": []
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("hi"))
        .await
        .expect_err("empty");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_missing_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{ "finishReason": "SAFETY", "index": 0 }]
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("hi"))
        .await
        .expect_err("no content");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_no_inline_data_part() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": { "role": "model", "parts": [ { "text": "not an image" } ] },
                "finishReason": "STOP",
                "index": 0
            }]
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("hi"))
        .await
        .expect_err("no inline");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("hi"))
        .await
        .expect_err("parse");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_uses_reference_images_and_grounding() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": { "role": "model", "parts": [
                    { "inlineData": { "mimeType": "image/png", "data": "YY" } }
                ]},
                "finishReason": "STOP",
                "index": 0
            }]
        })))
        .mount(&server)
        .await;
    let p = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("with refs");
    req.enable_search_grounding = true;
    req.reference_images = vec![
        systemprompt_ai::models::image_generation::ReferenceImage {
            data: "ZZZ".to_owned(),
            mime_type: "image/png".to_owned(),
            description: Some("ref".to_owned()),
        },
        systemprompt_ai::models::image_generation::ReferenceImage {
            data: "AAA".to_owned(),
            mime_type: "image/jpeg".to_owned(),
            description: None,
        },
    ];
    let resp = p.generate_image(&req).await.expect("ok");
    assert_eq!(resp.image_data, "YY");
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

// A model definition that opts into resolution configuration. Without one the
// provider omits the image-size block entirely, so the resolution mapping
// never runs.
fn resolution_capable_models(model: &str) -> std::collections::HashMap<String, ModelDefinition> {
    let mut definitions = std::collections::HashMap::new();
    definitions.insert(
        model.to_owned(),
        ModelDefinition {
            capabilities: ModelCapabilities {
                image_resolution_config: true,
                ..ModelCapabilities::default()
            },
            ..ModelDefinition::default()
        },
    );
    definitions
}

async fn generate_at(resolution: ImageResolution) -> String {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "inlineData": { "mimeType": "image/png", "data": "AAAA" } }]
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": { "promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2 }
        })))
        .mount(&server)
        .await;

    let model = "gemini-2.5-flash-image";
    let provider = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri())
        .with_model_definitions(resolution_capable_models(model));

    let mut request = make_request("a picture");
    request.resolution = resolution;
    request.model = Some(model.to_owned());
    provider
        .generate_image(&request)
        .await
        .expect("generation succeeds");

    let received = server.received_requests().await.expect("recorded requests");
    String::from_utf8(received[0].body.clone()).expect("request body is utf8")
}

#[tokio::test]
async fn each_resolution_is_sent_upstream_in_geminis_own_vocabulary() {
    for (resolution, expected) in [
        (ImageResolution::OneK, "1K"),
        (ImageResolution::TwoK, "2K"),
        (ImageResolution::FourK, "4K"),
    ] {
        let body = generate_at(resolution).await;
        assert!(
            body.contains(expected),
            "the {expected} request must carry Gemini's own size token, got {body}"
        );
    }
}

#[tokio::test]
async fn a_model_that_does_not_declare_resolution_support_omits_the_size_block() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r".*/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "inlineData": { "mimeType": "image/png", "data": "AAAA" } }]
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": { "promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2 }
        })))
        .mount(&server)
        .await;

    // No model definitions at all: the provider must not invent a capability
    // the upstream model does not have.
    let provider = GeminiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut request = make_request("a picture");
    request.resolution = ImageResolution::FourK;
    provider
        .generate_image(&request)
        .await
        .expect("generation succeeds without resolution configuration");

    let received = server.received_requests().await.expect("recorded requests");
    let body = String::from_utf8(received[0].body.clone()).expect("utf8");
    assert!(
        !body.contains("4K"),
        "an undeclared capability must not be sent upstream, got {body}"
    );
}
