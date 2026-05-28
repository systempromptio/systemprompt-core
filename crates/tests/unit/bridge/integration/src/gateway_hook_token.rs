//! Round-trips `mint_plugin_hook_token` against a wiremock gateway to lock
//! in the new `HookTokenRejected` error shape: any non-2xx must surface the
//! gateway's response body so `bridge sync` PARTIAL lines are
//! self-diagnosing instead of swallowing the error JSON behind a bare
//! status code.

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::errors::GatewayError;
use systemprompt_identifiers::{ClientId, PluginId, ValidatedUrl};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn gateway_for(server: &MockServer) -> GatewayClient {
    let base = ValidatedUrl::try_new(server.uri()).expect("valid wiremock uri");
    GatewayClient::new(base)
}

#[tokio::test(flavor = "multi_thread")]
async fn hook_token_401_captures_body() {
    let server = MockServer::start().await;
    let body = r#"{"error":"invalid_client","error_description":"Client owner is not active"}"#;
    Mock::given(method("POST"))
        .and(path("/v1/oauth/token"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let gateway = gateway_for(&server);
    let token_endpoint = format!("{}/v1/oauth/token", server.uri());
    let err = gateway
        .mint_plugin_hook_token(
            &token_endpoint,
            &ClientId::new("test-client"),
            "test-secret",
            &PluginId::new("plugin-a"),
        )
        .await
        .expect_err("401 must surface as an error");

    match err {
        GatewayError::HookTokenRejected { status, body: got } => {
            assert_eq!(status.as_u16(), 401);
            assert!(
                got.contains("invalid_client"),
                "body must be propagated for operator-visible diagnosis, got: {got}"
            );
        },
        other => panic!("expected HookTokenRejected with body, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hook_token_500_captures_body() {
    let server = MockServer::start().await;
    let body =
        r#"{"error":"server_error","error_description":"Failed to load client owner: db down"}"#;
    Mock::given(method("POST"))
        .and(path("/v1/oauth/token"))
        .respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let gateway = gateway_for(&server);
    let token_endpoint = format!("{}/v1/oauth/token", server.uri());
    let err = gateway
        .mint_plugin_hook_token(
            &token_endpoint,
            &ClientId::new("test-client"),
            "test-secret",
            &PluginId::new("plugin-a"),
        )
        .await
        .expect_err("500 must surface as an error");

    match err {
        GatewayError::HookTokenRejected { status, body: got } => {
            assert_eq!(status.as_u16(), 500);
            assert!(got.contains("server_error"), "body propagated: {got}");
        },
        other => panic!("expected HookTokenRejected with body, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hook_token_200_returns_token() {
    let server = MockServer::start().await;
    let body = r#"{"access_token":"jwt.value","token_type":"Bearer","expires_in":3600,"scope":"hook:govern hook:track"}"#;
    Mock::given(method("POST"))
        .and(path("/v1/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let gateway = gateway_for(&server);
    let token_endpoint = format!("{}/v1/oauth/token", server.uri());
    let response = gateway
        .mint_plugin_hook_token(
            &token_endpoint,
            &ClientId::new("test-client"),
            "test-secret",
            &PluginId::new("plugin-a"),
        )
        .await
        .expect("happy path must succeed");

    assert_eq!(response.access_token, "jwt.value");
    assert_eq!(response.expires_in, 3600);
}
