//! Tests for the auth-mutating `GatewayClient` endpoints: mTLS/session/PAT
//! exchanges, OAuth client provisioning, and per-plugin hook token minting.
//! Each test programs a `wiremock` mock and asserts either the decoded success
//! payload or the specific `GatewayError` variant.

use systemprompt_bridge::auth::types::{MtlsRequest, SessionExchangeRequest, SessionPatRequest};
use systemprompt_bridge::gateway::{GatewayClient, GatewayError};
use systemprompt_bridge::ids::CertFingerprint;
use systemprompt_identifiers::{ClientId, PluginId, SessionId, ValidatedUrl};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> GatewayClient {
    GatewayClient::new(ValidatedUrl::new(server.uri()))
}

fn session_id() -> SessionId {
    SessionId::generate()
}

fn auth_body() -> serde_json::Value {
    serde_json::json!({
        "token": "jwt.abc.def",
        "ttl": 900,
        "headers": { "x-sp-user": "user_1" }
    })
}

#[tokio::test]
async fn mtls_exchange_decodes_auth_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/mtls"))
        .and(body_string_contains("device_cert_fingerprint"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_body()))
        .mount(&server)
        .await;

    let req = MtlsRequest {
        device_cert_fingerprint: CertFingerprint::try_new("a".repeat(64)).unwrap(),
    };
    let out = client(&server)
        .mtls_exchange(&req, &session_id())
        .await
        .unwrap();
    assert_eq!(out.ttl, 900);
    assert_eq!(out.token.expose(), "jwt.abc.def");
    assert_eq!(out.headers.len(), 1);
}

#[tokio::test]
async fn mtls_exchange_403_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/mtls"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let req = MtlsRequest {
        device_cert_fingerprint: CertFingerprint::try_new("b".repeat(64)).unwrap(),
    };
    let err = client(&server)
        .mtls_exchange(&req, &session_id())
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 403);
            assert_eq!(endpoint, "mtls");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn session_exchange_decodes_auth_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session"))
        .and(body_string_contains("one-time-code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_body()))
        .mount(&server)
        .await;

    let req = SessionExchangeRequest {
        code: "one-time-code".into(),
    };
    let out = client(&server)
        .session_exchange(&req, &session_id())
        .await
        .unwrap();
    assert_eq!(out.ttl, 900);
}

#[tokio::test]
async fn session_exchange_malformed_body_maps_to_auth_decode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("not json", "application/json"))
        .mount(&server)
        .await;

    let req = SessionExchangeRequest { code: "c".into() };
    let err = client(&server)
        .session_exchange(&req, &session_id())
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::AuthDecode(_)),
        "expected AuthDecode, got {err:?}"
    );
}

#[tokio::test]
async fn session_pat_exchange_returns_pat() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "pat": "sp-live-minted" })),
        )
        .mount(&server)
        .await;

    let req = SessionPatRequest {
        code: "code-1".into(),
        device_name: Some("test-box".into()),
    };
    let pat = client(&server)
        .session_pat_exchange(&req, &session_id())
        .await
        .unwrap();
    assert_eq!(pat, "sp-live-minted");
}

#[tokio::test]
async fn session_pat_exchange_401_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let req = SessionPatRequest {
        code: "expired".into(),
        device_name: None,
    };
    let err = client(&server)
        .session_pat_exchange(&req, &session_id())
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 401);
            assert_eq!(endpoint, "session-pat");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn session_pat_exchange_malformed_body_maps_to_auth_decode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/session-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
        .mount(&server)
        .await;

    let req = SessionPatRequest {
        code: "c".into(),
        device_name: None,
    };
    let err = client(&server)
        .session_pat_exchange(&req, &session_id())
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::AuthDecode(_)),
        "expected AuthDecode, got {err:?}"
    );
}

#[tokio::test]
async fn pat_exchange_sends_bearer_and_decodes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .and(header("authorization", "Bearer sp-live-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_body()))
        .mount(&server)
        .await;

    let out = client(&server)
        .pat_exchange("sp-live-abc", &session_id())
        .await
        .unwrap();
    assert_eq!(out.token.expose(), "jwt.abc.def");
}

#[tokio::test]
async fn pat_exchange_401_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let err = client(&server)
        .pat_exchange("sp-live-revoked", &session_id())
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 401);
            assert_eq!(endpoint, "pat");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn pat_exchange_malformed_body_maps_to_auth_decode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    let err = client(&server)
        .pat_exchange("sp-live-abc", &session_id())
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::AuthDecode(_)),
        "expected AuthDecode, got {err:?}"
    );
}

#[tokio::test]
async fn provision_oauth_client_decodes_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/oauth-client"))
        .and(header("authorization", "Bearer sp-live-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "client_id": "client_1",
            "client_secret": "s3cret",
            "scopes": ["hook:govern"],
            "token_endpoint": format!("{}/oauth/token", server.uri()),
        })))
        .mount(&server)
        .await;

    let out = client(&server)
        .provision_oauth_client("sp-live-abc")
        .await
        .unwrap();
    assert_eq!(out.client_id.as_str(), "client_1");
    assert_eq!(out.client_secret, "s3cret");
    assert_eq!(out.scopes, vec!["hook:govern".to_owned()]);
}

#[tokio::test]
async fn provision_oauth_client_500_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/oauth-client"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client(&server)
        .provision_oauth_client("sp-live-abc")
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 500);
            assert_eq!(endpoint, "oauth-client");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn provision_oauth_client_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/oauth-client"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("nope", "application/json"))
        .mount(&server)
        .await;

    let err = client(&server)
        .provision_oauth_client("sp-live-abc")
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::OAuthClientDecode(_)),
        "expected OAuthClientDecode, got {err:?}"
    );
}

#[tokio::test]
async fn mint_plugin_hook_token_sends_client_credentials_form() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("plugin_id=plugin-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "hook.jwt",
            "token_type": "Bearer",
            "expires_in": 600,
            "scope": "hook:govern hook:track",
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/oauth/token", server.uri());
    let out = client(&server)
        .mint_plugin_hook_token(
            &endpoint,
            &ClientId::new("client_1"),
            "s3cret",
            &PluginId::new("plugin-a"),
        )
        .await
        .unwrap();
    assert_eq!(out.access_token, "hook.jwt");
    assert_eq!(out.expires_in, 600);
    assert_eq!(out.token_type.as_deref(), Some("Bearer"));
}

#[tokio::test]
async fn mint_plugin_hook_token_rejection_carries_status_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(403).set_body_raw("scope denied", "text/plain"))
        .mount(&server)
        .await;

    let endpoint = format!("{}/oauth/token", server.uri());
    let err = client(&server)
        .mint_plugin_hook_token(
            &endpoint,
            &ClientId::new("client_1"),
            "bad",
            &PluginId::new("plugin-a"),
        )
        .await
        .unwrap_err();
    match err {
        GatewayError::HookTokenRejected { status, body } => {
            assert_eq!(status.as_u16(), 403);
            assert_eq!(body, "scope denied");
        },
        other => panic!("expected HookTokenRejected, got {other:?}"),
    }
}

#[tokio::test]
async fn mint_plugin_hook_token_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("nope", "application/json"))
        .mount(&server)
        .await;

    let endpoint = format!("{}/oauth/token", server.uri());
    let err = client(&server)
        .mint_plugin_hook_token(
            &endpoint,
            &ClientId::new("client_1"),
            "s3cret",
            &PluginId::new("plugin-a"),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::HookTokenDecode(_)),
        "expected HookTokenDecode, got {err:?}"
    );
}
