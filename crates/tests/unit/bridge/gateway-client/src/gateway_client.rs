//! Integration tests for `GatewayClient` against a `wiremock` mock server.
//!
//! Each test stands up a `MockServer`, programs the exact endpoint the client
//! hits, builds a `GatewayClient` pointed at the mock's URI, and asserts both
//! the success-decode path and the relevant `GatewayError` variant on failure.

use systemprompt_bridge::gateway::{GatewayClient, GatewayError};
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> GatewayClient {
    GatewayClient::new(ValidatedUrl::new(server.uri()))
}

const BEARER: &str = "test-bearer-token";

fn manifest_json() -> serde_json::Value {
    serde_json::json!({
        "manifest_version": "2026-06-03T00:00:00Z-deadbeef",
        "issued_at": "2026-06-03T00:00:00Z",
        "not_before": "2026-06-03T00:00:00Z",
        "user_id": "user_abc",
        "tenant_id": null,
        "plugins": [],
        "managed_mcp_servers": [],
        "revocations": [],
        "signature": ""
    })
}

#[tokio::test]
async fn health_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let result = client(&server).health().await;
    assert!(result.is_ok(), "expected Ok, got {result:?}");
}

#[tokio::test]
async fn health_503_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let err = client(&server).health().await.unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 503);
            assert_eq!(endpoint, "health");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_pubkey_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/pubkey"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "pubkey": "ZmFrZS1wdWJrZXktYjY0"
        })))
        .mount(&server)
        .await;

    let pubkey = client(&server).fetch_pubkey().await.unwrap();
    assert_eq!(pubkey, "ZmFrZS1wdWJrZXktYjY0");
}

#[tokio::test]
async fn fetch_pubkey_missing_field_maps_to_pubkey_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/pubkey"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    let err = client(&server).fetch_pubkey().await.unwrap_err();
    assert!(
        matches!(err, GatewayError::PubkeyMissing),
        "expected PubkeyMissing, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_pubkey_404_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/pubkey"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = client(&server).fetch_pubkey().await.unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 404);
            assert_eq!(endpoint, "pubkey");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_pubkey_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/pubkey"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let err = client(&server).fetch_pubkey().await.unwrap_err();
    assert!(
        matches!(err, GatewayError::PubkeyDecode(_)),
        "expected PubkeyDecode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_manifest_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .and(header("authorization", format!("Bearer {BEARER}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json()))
        .mount(&server)
        .await;

    let manifest = client(&server).fetch_manifest(BEARER).await.unwrap();
    assert_eq!(manifest.user_id.as_str(), "user_abc");
    assert!(manifest.plugins.is_empty());
}

#[tokio::test]
async fn fetch_manifest_401_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let err = client(&server).fetch_manifest(BEARER).await.unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 401);
            assert_eq!(endpoint, "manifest");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_manifest_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{ not a manifest }"))
        .mount(&server)
        .await;

    let err = client(&server).fetch_manifest(BEARER).await.unwrap_err();
    assert!(
        matches!(err, GatewayError::ManifestDecode(_)),
        "expected ManifestDecode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_whoami_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .and(header("authorization", format!("Bearer {BEARER}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "user_id": "user_abc",
            "email": "ed@example.com",
            "roles": ["admin", "member"]
        })))
        .mount(&server)
        .await;

    let whoami = client(&server).fetch_whoami(BEARER).await.unwrap();
    assert_eq!(whoami.email.as_deref(), Some("ed@example.com"));
    assert_eq!(whoami.roles, vec!["admin".to_owned(), "member".to_owned()]);
}

#[tokio::test]
async fn fetch_whoami_403_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let err = client(&server).fetch_whoami(BEARER).await.unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 403);
            assert_eq!(endpoint, "whoami");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_whoami_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_string("nope"))
        .mount(&server)
        .await;

    let err = client(&server).fetch_whoami(BEARER).await.unwrap_err();
    assert!(
        matches!(err, GatewayError::WhoamiDecode(_)),
        "expected WhoamiDecode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_bridge_profile_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "inference_gateway_base_url": "https://gw.example.com",
            "auth_scheme": "bearer",
            "models": ["claude-x", "gpt-y"]
        })))
        .mount(&server)
        .await;

    let profile = client(&server).fetch_bridge_profile().await.unwrap();
    assert_eq!(profile.inference_gateway_base_url, "https://gw.example.com");
    assert_eq!(profile.auth_scheme, "bearer");
    assert_eq!(
        profile.models,
        vec!["claude-x".to_owned(), "gpt-y".to_owned()]
    );
}

#[tokio::test]
async fn fetch_bridge_profile_500_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = client(&server).fetch_bridge_profile().await.unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 500);
            assert_eq!(endpoint, "profile");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_bridge_profile_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{"))
        .mount(&server)
        .await;

    let err = client(&server).fetch_bridge_profile().await.unwrap_err();
    assert!(
        matches!(err, GatewayError::ProfileDecode(_)),
        "expected ProfileDecode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_profile_usage_ok() {
    let server = MockServer::start().await;
    let window = serde_json::json!({
        "requests": 0,
        "tokens": 0,
        "cost_microdollars": 0
    });
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile/usage"))
        .and(header("authorization", format!("Bearer {BEARER}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "d1": window,
            "d7": serde_json::json!({ "requests": 5, "tokens": 100, "cost_microdollars": 42 }),
            "d30": window
        })))
        .mount(&server)
        .await;

    let usage = client(&server).fetch_profile_usage(BEARER).await.unwrap();
    assert_eq!(usage.d7.requests, 5);
    assert_eq!(usage.d7.cost_microdollars, 42);
}

#[tokio::test]
async fn fetch_profile_usage_502_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile/usage"))
        .respond_with(ResponseTemplate::new(502))
        .mount(&server)
        .await;

    let err = client(&server)
        .fetch_profile_usage(BEARER)
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 502);
            assert_eq!(endpoint, "profile_usage");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_profile_usage_malformed_body_maps_to_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/profile/usage"))
        .respond_with(ResponseTemplate::new(200).set_body_string("garbage"))
        .mount(&server)
        .await;

    let err = client(&server)
        .fetch_profile_usage(BEARER)
        .await
        .unwrap_err();
    assert!(
        matches!(err, GatewayError::ProfileUsageDecode(_)),
        "expected ProfileUsageDecode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_plugin_file_ok() {
    let server = MockServer::start().await;
    let payload = b"plugin file bytes".to_vec();
    Mock::given(method("GET"))
        .and(path("/v1/bridge/plugins/my-plugin/dist/index.js"))
        .and(header("authorization", format!("Bearer {BEARER}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .mount(&server)
        .await;

    let bytes = client(&server)
        .fetch_plugin_file(BEARER, "my-plugin", "dist/index.js")
        .await
        .unwrap();
    assert_eq!(bytes, payload);
}

#[tokio::test]
async fn fetch_plugin_file_404_maps_to_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/plugins/my-plugin/missing.js"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = client(&server)
        .fetch_plugin_file(BEARER, "my-plugin", "missing.js")
        .await
        .unwrap_err();
    match err {
        GatewayError::HttpStatus { status, endpoint } => {
            assert_eq!(status.as_u16(), 404);
            assert_eq!(endpoint, "plugin");
        },
        other => panic!("expected HttpStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_plugin_file_traversal_path_maps_to_unsafe_path() {
    let server = MockServer::start().await;
    let err = client(&server)
        .fetch_plugin_file(BEARER, "my-plugin", "../etc/passwd")
        .await
        .unwrap_err();
    match err {
        GatewayError::UnsafePath(p) => assert_eq!(p, "../etc/passwd"),
        other => panic!("expected UnsafePath, got {other:?}"),
    }
}
