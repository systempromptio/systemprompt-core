//! Revalidating fetches: the manifest under `If-None-Match`, and the plugin
//! archive's 404 (older gateway) and 409 (stale digest) answers.

use systemprompt_bridge::gateway::{ArchiveResponse, Freshness, GatewayClient, ManifestResponse};
use systemprompt_bridge::ids::{BearerToken, PluginId, Sha256Digest};
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new(server.uri()).unwrap(),
        reqwest::Client::new(),
    )
}

fn bearer() -> BearerToken {
    BearerToken::new("test-bearer-token")
}

fn envelope() -> serde_json::Value {
    serde_json::json!({ "payload": "{}", "signature": "" })
}

#[tokio::test]
async fn a_fresh_manifest_returns_its_etag_and_asks_for_schema_two() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .and(header("x-bridge-manifest-schema", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"v1\"")
                .set_body_json(envelope()),
        )
        .mount(&server)
        .await;
    let response = client(&server)
        .fetch_manifest_conditional(&bearer(), Freshness::Memo, None)
        .await
        .unwrap();
    match response {
        ManifestResponse::Fresh { etag, .. } => assert_eq!(etag.as_deref(), Some("\"v1\"")),
        other => panic!("expected Fresh, got {other:?}"),
    }
}

#[tokio::test]
async fn a_304_for_the_sent_etag_is_not_modified() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .and(header("if-none-match", "\"v1\""))
        .and(header("cache-control", "no-cache"))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    let response = client(&server)
        .fetch_manifest_conditional(&bearer(), Freshness::Fresh, Some("\"v1\""))
        .await
        .unwrap();
    assert!(
        matches!(response, ManifestResponse::NotModified),
        "{response:?}"
    );
}

#[tokio::test]
async fn a_304_without_a_sent_etag_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/manifest"))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    let result = client(&server)
        .fetch_manifest_conditional(&bearer(), Freshness::Memo, None)
        .await;
    assert!(result.is_err(), "{result:?}");
}

async fn archive_status(status: u16) -> ArchiveResponse {
    let server = MockServer::start().await;
    let sha = "a".repeat(64);
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/bridge/plugins/acme-plugin/archive/{sha}"
        )))
        .respond_with(ResponseTemplate::new(status).set_body_bytes(b"tgz".to_vec()))
        .mount(&server)
        .await;
    client(&server)
        .fetch_plugin_archive(
            &bearer(),
            &PluginId::try_new("acme-plugin").unwrap(),
            &Sha256Digest::try_new(sha).unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn an_archive_is_returned_verbatim() {
    match archive_status(200).await {
        ArchiveResponse::Archive(bytes) => assert_eq!(bytes, b"tgz"),
        other => panic!("expected Archive, got {other:?}"),
    }
}

#[tokio::test]
async fn an_archive_404_means_an_older_gateway() {
    assert!(matches!(
        archive_status(404).await,
        ArchiveResponse::Unsupported
    ));
}

#[tokio::test]
async fn an_archive_409_means_a_stale_manifest() {
    assert!(matches!(archive_status(409).await, ArchiveResponse::Stale));
}
