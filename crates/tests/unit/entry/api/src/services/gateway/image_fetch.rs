//! Resolution of caller-supplied image URLs to inline base64, against a
//! wiremock'd image host. Covers the happy path, every bound the fetcher
//! enforces (timeout, size, content type), and the SSRF guard including a
//! redirect to an internal address.

use std::time::Duration;

use serde_json::Value;
use systemprompt_api::services::gateway::image_fetch::{
    ImageFetchPolicy, MAX_IMAGE_BYTES, fetch, inline_url_images,
};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, ImageSource, Role,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-pixels";

/// Loopback is blocked for caller-supplied URLs, so a mock host has to be
/// trusted explicitly — which is exactly the assertion the SSRF tests make by
/// leaving the list empty.
fn policy_trusting_mock() -> ImageFetchPolicy {
    ImageFetchPolicy {
        trusted_hosts: vec!["127.0.0.1".to_owned()],
        ..ImageFetchPolicy::default()
    }
}

fn url_image_request(url: &str) -> CanonicalRequest {
    CanonicalRequest {
        model: "gemini-2.5-pro".into(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![
                CanonicalContent::Text("what is this?".into()),
                CanonicalContent::Image(ImageSource::Url {
                    url: url.to_owned(),
                    detail: None,
                }),
            ],
        }],
        ..CanonicalRequest::default()
    }
}

async fn image_host(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/pic.png"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn png_response() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "image/png")
        .set_body_bytes(PNG.to_vec())
}

#[tokio::test]
async fn url_image_is_fetched_and_inlined() {
    let server = image_host(png_response()).await;
    let mut request = url_image_request(&format!("{}/pic.png", server.uri()));

    let count = inline_url_images(&mut request, &policy_trusting_mock())
        .await
        .expect("inline succeeds");

    assert_eq!(count, 1);
    let inlined = &request.messages[0].content[1];
    let CanonicalContent::Image(ImageSource::Base64 {
        media_type, data, ..
    }) = inlined
    else {
        panic!("url image was not rewritten to inline data: {inlined:?}");
    };
    assert_eq!(media_type, "image/png");
    assert_eq!(
        data,
        &base64::Engine::encode(&base64::engine::general_purpose::STANDARD, PNG)
    );
}

#[tokio::test]
async fn inlined_image_reaches_the_gemini_wire_as_inline_data() {
    let server = image_host(png_response()).await;
    let mut request = url_image_request(&format!("{}/pic.png", server.uri()));
    inline_url_images(&mut request, &policy_trusting_mock())
        .await
        .expect("inline succeeds");

    let body: Value = systemprompt_models::wire::gemini::build_request_body(&request, None);
    let part = &body["contents"][0]["parts"][1];

    assert_eq!(part["inlineData"]["mimeType"], "image/png");
    assert!(part.get("text").is_none(), "still a text part: {part}");
}

#[tokio::test]
async fn a_hung_host_times_out_rather_than_hanging_the_request() {
    let server = image_host(png_response().set_delay(Duration::from_secs(30))).await;
    let policy = ImageFetchPolicy {
        timeout: Duration::from_millis(150),
        ..policy_trusting_mock()
    };

    let failure = fetch(&format!("{}/pic.png", server.uri()), &policy)
        .await
        .expect_err("a hung host must not be waited on");

    assert!(failure.message.contains("exceeded"), "{failure:?}");
    assert!(
        !failure.caller_fault,
        "a slow host is not the caller's fault"
    );
}

#[tokio::test]
async fn an_oversize_body_is_rejected() {
    let server = image_host(
        ResponseTemplate::new(200)
            .insert_header("content-type", "image/png")
            .set_body_bytes(vec![0_u8; 4096]),
    )
    .await;
    let policy = ImageFetchPolicy {
        max_bytes: 512,
        ..policy_trusting_mock()
    };

    let failure = fetch(&format!("{}/pic.png", server.uri()), &policy)
        .await
        .expect_err("oversize image must be refused");

    assert!(failure.message.contains("larger than 512"), "{failure:?}");
    assert!(failure.caller_fault);
}

#[tokio::test]
async fn a_non_image_content_type_is_rejected() {
    let server = image_host(
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/html; charset=utf-8")
            .set_body_bytes(b"<html>not a picture</html>".to_vec()),
    )
    .await;

    let failure = fetch(
        &format!("{}/pic.png", server.uri()),
        &policy_trusting_mock(),
    )
    .await
    .expect_err("a .png url serving html must be refused");

    assert!(failure.message.contains("text/html"), "{failure:?}");
    assert!(failure.caller_fault);
}

#[tokio::test]
async fn loopback_is_refused_when_it_is_not_an_operator_trusted_host() {
    let server = image_host(png_response()).await;
    let policy = ImageFetchPolicy {
        trusted_hosts: Vec::new(),
        ..ImageFetchPolicy::default()
    };

    let failure = fetch(&format!("{}/pic.png", server.uri()), &policy)
        .await
        .expect_err("a caller must not be able to make the server fetch itself");

    assert!(failure.caller_fault, "{failure:?}");
}

#[tokio::test]
async fn a_redirect_to_cloud_metadata_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/pic.png"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "https://169.254.169.254/latest/meta-data/"),
        )
        .mount(&server)
        .await;

    let failure = fetch(
        &format!("{}/pic.png", server.uri()),
        &policy_trusting_mock(),
    )
    .await
    .expect_err("a redirect into the link-local range must be refused");

    assert!(failure.message.contains("redirect rejected"), "{failure:?}");
    assert!(failure.message.contains("169.254.169.254"), "{failure:?}");
    assert!(failure.caller_fault);
}

#[tokio::test]
async fn a_private_range_host_is_refused_without_any_request_being_made() {
    let failure = fetch("https://10.0.0.5/pic.png", &ImageFetchPolicy::default())
        .await
        .expect_err("private ranges are not fetchable");

    assert!(failure.caller_fault, "{failure:?}");
}

#[tokio::test]
async fn wires_that_carry_urls_natively_are_left_alone() {
    // Why: the fetch is Gemini-only, so the other wires must still render a URL
    // image straight through. No mock host is started: reaching the network at
    // all here would be the regression.
    let request = url_image_request("https://example.com/pic.png");

    let anthropic =
        systemprompt_models::wire::anthropic::build_request_body(&request, "claude-x", None);
    let rendered = serde_json::to_string(&anthropic).expect("serialises");

    assert!(
        rendered.contains("https://example.com/pic.png"),
        "{rendered}"
    );
}

#[test]
fn the_size_cap_stays_under_the_gemini_inline_budget() {
    // Why: base64 inflates by 4/3, and Gemini caps the whole request at 20 MB.
    assert!(MAX_IMAGE_BYTES * 4 / 3 < 20 * 1000 * 1000);
}
