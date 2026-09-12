//! Tests for the external-MCP bearer accessor seam: accessor URL assembly,
//! bearer fetch against a scripted accessor endpoint, and outbound header
//! construction.

use std::collections::HashMap;

use systemprompt_mcp::services::client::external_auth::{
    accessor_url, fetch_external_bearer, outbound_headers, static_outbound_headers,
};
use systemprompt_models::mcp::ExternalAuth;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ext_auth(endpoint: &str) -> ExternalAuth {
    serde_yaml::from_str(&format!("token_endpoint: {endpoint}")).expect("external auth yaml")
}

#[test]
fn accessor_url_joins_base_and_endpoint() {
    let url = accessor_url("https://api.example.com/", "/api/public/prov/token");
    assert!(url.ends_with("/api/public/prov/token"));
    assert!(!url.contains("com//api"));
}

#[test]
fn ext_auth_defaults_apply() {
    let ext = ext_auth("/api/public/prov/token");
    assert_eq!(ext.header, "Authorization");
    assert_eq!(ext.header_value("tok"), "Bearer tok");
}

#[tokio::test]
async fn fetch_bearer_returns_access_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(header("authorization", "Bearer my-jwt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"access_token": "banked-token"})),
        )
        .mount(&server)
        .await;

    let bearer = fetch_external_bearer(&format!("{}/token", server.uri()), "my-jwt", None, "srv")
        .await
        .expect("bearer resolves");
    assert_eq!(bearer, "banked-token");
}

#[tokio::test]
async fn fetch_bearer_rejects_empty_access_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"access_token": "  "})),
        )
        .mount(&server)
        .await;

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", None, "srv")
        .await
        .expect_err("empty token rejected");
    assert!(err.to_string().contains("empty access_token"));
}

#[tokio::test]
async fn fetch_bearer_maps_not_found_to_unconnected_account() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", None, "srv")
        .await
        .expect_err("404 surfaces");
    assert!(err.to_string().contains("no token banked"));
}

#[tokio::test]
async fn fetch_bearer_surfaces_other_statuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", None, "srv")
        .await
        .expect_err("503 surfaces");
    assert!(err.to_string().contains("503"));
}

#[tokio::test]
async fn fetch_bearer_rejects_unreadable_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("not-json", "application/json"))
        .mount(&server)
        .await;

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", None, "srv")
        .await
        .expect_err("bad body surfaces");
    assert!(err.to_string().contains("unreadable body"));
}

#[tokio::test]
async fn fetch_bearer_maps_transport_failure() {
    let err = fetch_external_bearer("http://127.0.0.1:1/token", "jwt", None, "srv")
        .await
        .expect_err("connection refused surfaces");
    assert!(err.to_string().contains("token accessor request failed"));
}

#[test]
fn outbound_headers_inject_bearer_over_static() {
    let ext = ext_auth("/api/public/prov/token");
    let mut statics = HashMap::new();
    statics.insert("x-region".to_owned(), "eu".to_owned());

    let out = outbound_headers(&ext, "tok-123", &statics, "srv").expect("headers build");
    assert_eq!(out.len(), 2);
    assert_eq!(
        out.get(&http::HeaderName::from_static("authorization"))
            .map(|v| v.to_str().unwrap()),
        Some("Bearer tok-123")
    );
    assert_eq!(
        out.get(&http::HeaderName::from_static("x-region"))
            .map(|v| v.to_str().unwrap()),
        Some("eu")
    );
}

#[test]
fn static_headers_reject_invalid_name() {
    let mut statics = HashMap::new();
    statics.insert("bad header".to_owned(), "v".to_owned());
    let err = static_outbound_headers(&statics, "srv").expect_err("invalid name rejected");
    assert!(err.to_string().contains("invalid header name"));
}

#[test]
fn static_headers_reject_invalid_value() {
    let mut statics = HashMap::new();
    statics.insert("x-ok".to_owned(), "line\nbreak".to_owned());
    let err = static_outbound_headers(&statics, "srv").expect_err("invalid value rejected");
    assert!(err.to_string().contains("invalid value"));
}

#[tokio::test]
async fn fetch_bearer_authenticates_the_broker() {
    const SECRET: &str = "test-broker-credential";
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(header("authorization", "Bearer employee-token"))
        .and(header("x-systemprompt-credential-broker", SECRET))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"access_token":"provider-token"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let bearer = fetch_external_bearer(
        &format!("{}/token", server.uri()),
        "employee-token",
        Some(SECRET),
        "provider",
    )
    .await
    .expect("broker authenticated");
    assert_eq!(bearer, "provider-token");
}

#[tokio::test]
async fn fetch_bearer_omits_the_broker_header_without_a_secret() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(header("authorization", "Bearer employee-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"access_token":"provider-token"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let bearer = fetch_external_bearer(
        &format!("{}/token", server.uri()),
        "employee-token",
        None,
        "provider",
    )
    .await
    .expect("accessor answered");
    assert_eq!(bearer, "provider-token");
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests[0]
            .headers
            .get("x-systemprompt-credential-broker")
            .is_none()
    );
}

#[tokio::test]
async fn fetch_bearer_does_not_forward_credentials_to_redirects() {
    let accessor = MockServer::start().await;
    let destination = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"access_token":"stolen"})),
        )
        .expect(0)
        .mount(&destination)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", destination.uri()))
        .mount(&accessor)
        .await;
    let error = fetch_external_bearer(&accessor.uri(), "employee-token", None, "provider")
        .await
        .expect_err("redirect refused");
    assert!(error.to_string().contains("302"));
}
