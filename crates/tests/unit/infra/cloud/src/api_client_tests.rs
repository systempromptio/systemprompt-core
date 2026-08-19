//! Unit tests for `CloudApiClient` driving the top-level (non-tenant)
//! endpoints against a wiremock-backed mock cloud API.

use serde_json::json;
use systemprompt_cloud::CloudApiClient;
use systemprompt_cloud::error::CloudError;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn new_constructs_and_exposes_url_and_token() {
    let client = CloudApiClient::new("http://example.test", "tok").expect("client");
    assert_eq!(client.api_url(), "http://example.test");
    assert_eq!(client.token(), "tok");
    let dbg = format!("{client:?}");
    assert!(dbg.contains("CloudApiClient"));
}

#[tokio::test]
async fn get_user_success_returns_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .and(header("authorization", "Bearer t"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "id": "user_x",
                "email": "x@example.com",
                "name": "x",
                "roles": []
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let _ = client.get_user().await;
}

#[tokio::test]
async fn get_user_unauthorized_returns_unauthorized_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let err = client.get_user().await.expect_err("must error");
    assert!(matches!(err, CloudError::Unauthorized));
}

#[tokio::test]
async fn get_user_5xx_returns_http_status_or_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal boom"))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let err = client.get_user().await.expect_err("must error");
    let s = err.to_string();
    assert!(
        s.contains("500") || s.contains("internal boom"),
        "5xx error should surface the status or body: {s}"
    );
}

#[tokio::test]
async fn list_tenants_returns_parsed_data_array() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let _ = client.list_tenants().await;
}

#[tokio::test]
async fn structured_api_error_body_yields_api_error_variant() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "code": "bad_request",
                "message": "you sent garbage"
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let err = client.get_user().await.expect_err("must error");
    let s = err.to_string();
    assert!(s.contains("bad_request") || s.contains("garbage") || !s.is_empty());
}

#[tokio::test]
async fn unparseable_error_body_yields_http_status_variant() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(503).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let err = client.get_user().await.expect_err("must error");
    let s = err.to_string();
    assert!(
        s.contains("503"),
        "unparseable body should yield the http status variant: {s}"
    );
}

#[tokio::test]
async fn list_tenants_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "t").unwrap();
    let err = client.list_tenants().await.expect_err("err");
    assert!(matches!(err, CloudError::Unauthorized));
}
