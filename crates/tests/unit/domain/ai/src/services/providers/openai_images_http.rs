use serde_json::json;
use systemprompt_ai::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageResolution,
};
use systemprompt_ai::services::providers::image_provider_trait::ImageProvider;
use systemprompt_ai::services::providers::openai_images::OpenAiImageProvider;
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
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
async fn generate_image_returns_b64() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "b64_json": "AAAA" } ]
        })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let resp = p.generate_image(&make_request("ok")).await.expect("ok");
    assert_eq!(resp.image_data, "AAAA");
    assert_eq!(resp.mime_type, "image/png");
}

#[tokio::test]
async fn generate_image_rejects_long_prompt() {
    let server = MockServer::start().await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let huge = "x".repeat(5000);
    let err = p
        .generate_image(&make_request(&huge))
        .await
        .expect_err("too long");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_unsupported_resolution() {
    let server = MockServer::start().await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("ok");
    req.resolution = ImageResolution::FourK;
    let err = p.generate_image(&req).await.expect_err("bad res");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_unsupported_aspect() {
    let server = MockServer::start().await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("ok");
    req.aspect_ratio = AspectRatio::UltraWide;
    let err = p.generate_image(&req).await.expect_err("bad asp");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_rejects_unsupported_model() {
    let server = MockServer::start().await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("ok");
    req.model = Some("unknown-model".to_owned());
    let err = p.generate_image(&req).await.expect_err("bad model");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_handles_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_handles_missing_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("err");
    let _ = format!("{err}");
}

#[tokio::test]
async fn batch_iterates_over_requests() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "b64_json": "BB" } ]
        })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let resp = p
        .generate_batch(&[make_request("a"), make_request("b")])
        .await
        .expect("ok");
    assert_eq!(resp.len(), 2);
}

#[tokio::test]
async fn generate_image_handles_b64_none_entry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "b64_json": null } ]
        })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("missing");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_handles_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("parse");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_handles_connection_refused() {
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), "http://127.0.0.1:1".to_owned());
    let err = p
        .generate_image(&make_request("ok"))
        .await
        .expect_err("conn");
    let _ = format!("{err}");
}

#[tokio::test]
async fn generate_image_maps_size_for_portrait() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "b64_json": "PP" } ]
        })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("ok");
    req.aspect_ratio = AspectRatio::Portrait916;
    let resp = p.generate_image(&req).await.expect("ok");
    assert_eq!(resp.image_data, "PP");
}

#[tokio::test]
async fn generate_image_maps_size_for_landscape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "b64_json": "LL" } ]
        })))
        .mount(&server)
        .await;
    let p = OpenAiImageProvider::with_endpoint("k".to_owned(), server.uri());
    let mut req = make_request("ok");
    req.aspect_ratio = AspectRatio::Landscape169;
    let resp = p.generate_image(&req).await.expect("ok");
    assert_eq!(resp.image_data, "LL");
}

#[tokio::test]
async fn provider_metadata_is_consistent() {
    let p = OpenAiImageProvider::new("k".to_owned()).with_default_model("dall-e-3".to_owned());
    assert_eq!(p.name(), "openai-image");
    assert_eq!(p.default_model(), "dall-e-3");
    let caps = p.capabilities();
    assert!(caps.supports_image_editing);
    assert!(!caps.supports_search_grounding);
    assert!(p.supports_model("dall-e-3"));
    assert!(!p.supports_model("midjourney"));
    assert!(p.supports_resolution(&ImageResolution::OneK));
    assert!(!p.supports_resolution(&ImageResolution::TwoK));
    assert!(p.supports_aspect_ratio(&AspectRatio::Square));
    assert!(!p.supports_aspect_ratio(&AspectRatio::UltraWide));
}
