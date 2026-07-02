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

    let bearer = fetch_external_bearer(&format!("{}/token", server.uri()), "my-jwt", "srv")
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

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", "srv")
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

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", "srv")
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

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", "srv")
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

    let err = fetch_external_bearer(&format!("{}/token", server.uri()), "jwt", "srv")
        .await
        .expect_err("bad body surfaces");
    assert!(err.to_string().contains("unreadable body"));
}

#[tokio::test]
async fn fetch_bearer_maps_transport_failure() {
    let err = fetch_external_bearer("http://127.0.0.1:1/token", "jwt", "srv")
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
