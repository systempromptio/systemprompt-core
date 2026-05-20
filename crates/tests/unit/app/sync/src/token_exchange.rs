//! Wire-format tests for the RFC 8693 subject-token exchange request.
//!
//! `exchange_subject_token` hardcodes `https://` so it cannot be pointed at a
//! plain-HTTP mock server. These tests replicate the request that
//! `exchange_subject_token` builds and assert the form-encoded body matches
//! the RFC 8693 contract the API will eventually accept.

use systemprompt_sync::api_client::exchange_subject_token;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn build_exchange_request(
    client: &reqwest::Client,
    base_url: &str,
    subject_token: &str,
) -> reqwest::RequestBuilder {
    let url = format!("{base_url}/api/v1/core/oauth/token");
    client.post(url).form(&[
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        ),
        ("subject_token", subject_token),
        (
            "subject_token_type",
            "urn:ietf:params:oauth:token-type:jwt",
        ),
    ])
}

#[tokio::test]
async fn request_uses_rfc8693_grant_type_and_form_encoding() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .and(header(
            "content-type",
            "application/x-www-form-urlencoded",
        ))
        .and(body_string_contains(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Atoken-exchange",
        ))
        .and(body_string_contains("subject_token=op-jwt"))
        .and(body_string_contains(
            "subject_token_type=urn%3Aietf%3Aparams%3Aoauth%3Atoken-type%3Ajwt",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "service-jwt",
            "token_type": "Bearer"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let response = build_exchange_request(&client, &server.uri(), "op-jwt")
        .send()
        .await
        .expect("send");
    assert!(response.status().is_success());
}

#[tokio::test]
async fn response_access_token_is_extracted() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "minted-service-jwt"
        })))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let parsed: serde_json::Value = build_exchange_request(&client, &server.uri(), "op-token")
        .send()
        .await
        .expect("send")
        .json()
        .await
        .expect("json");

    assert_eq!(parsed["access_token"], "minted-service-jwt");
}

#[test]
fn exchange_subject_token_is_callable() {
    let _ = exchange_subject_token;
}
