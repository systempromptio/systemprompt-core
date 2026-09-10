//! The OCI transport against a stub registry.
//!
//! Covers the Bearer challenge round trip and the two ways a registry can
//! hand back the wrong bytes: an ambiguous layer set, and a blob whose
//! content does not match the digest the manifest promised.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use sha2::Digest;
use systemprompt_loader::bundle::BundleFetcher;
use systemprompt_loader::bundle::source::oci::OciFetcher;
use systemprompt_models::services::bundle::BUNDLE_MEDIA_TYPE;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BODY: &[u8] = b"bundle-bytes";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client")
}

fn digest_of(body: &[u8]) -> String {
    format!("sha256:{}", hex::encode(sha2::Sha256::digest(body)))
}

fn manifest_body(layers: serde_json::Value) -> serde_json::Value {
    json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.systemprompt.services-bundle.config.v1+json",
            "digest": digest_of(b"{}"),
            "size": 2
        },
        "layers": layers
    })
}

fn one_layer(body: &[u8]) -> serde_json::Value {
    json!([{ "mediaType": BUNDLE_MEDIA_TYPE, "digest": digest_of(body), "size": body.len() }])
}

async fn mount_manifest(server: &MockServer, layers: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("docker-content-digest", "sha256:deadbeef")
                .set_body_json(manifest_body(layers)),
        )
        .mount(server)
        .await;
}

fn fetcher(server: &MockServer) -> OciFetcher {
    let host = server.uri().replace("http://", "");
    OciFetcher::new("base", &format!("{host}/org/bundle:v1"), None, client()).expect("fetcher")
}

#[tokio::test]
async fn an_anonymous_pull_fetches_the_bundle_layer() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/org/bundle/blobs/{}", digest_of(BODY))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let fetched = fetcher(&server)
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect("pull");

    assert_eq!(std::fs::read(&fetched.archive).expect("read"), BODY);
    assert_eq!(fetched.digest, digest_of(BODY));
}

#[tokio::test]
async fn head_reports_the_manifest_digest() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;

    let remote = fetcher(&server).head().await.expect("head");

    assert_eq!(remote.digest, "sha256:deadbeef");
}

#[tokio::test]
async fn a_bearer_challenge_is_answered_and_the_request_retried() {
    let server = MockServer::start().await;
    let realm = format!("{}/token", server.uri());
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .and(header("authorization", "Bearer issued-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("docker-content-digest", "sha256:cafe")
                .set_body_json(manifest_body(one_layer(BODY))),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .respond_with(
            ResponseTemplate::new(401).insert_header(
                "www-authenticate",
                format!(
                    "Bearer realm=\"{realm}\",service=\"reg\",scope=\"repository:org/bundle:pull\""
                )
                .as_str(),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "issued-token"})))
        .mount(&server)
        .await;

    let remote = fetcher(&server).head().await.expect("head after challenge");

    assert_eq!(remote.digest, "sha256:cafe");
}

#[tokio::test]
async fn a_manifest_without_exactly_one_bundle_layer_is_refused() {
    let server = MockServer::start().await;
    mount_manifest(&server, json!([])).await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = fetcher(&server)
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect_err("an ambiguous manifest must be refused");

    assert!(
        err.to_string().contains("expected exactly one"),
        "the refusal names the ambiguity: {err}"
    );
}

#[tokio::test]
async fn a_blob_whose_content_does_not_match_its_digest_is_refused() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/org/bundle/blobs/{}", digest_of(BODY))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"substituted".as_slice()))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = fetcher(&server)
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect_err("a substituted blob must be refused");

    assert!(
        err.to_string().contains("blob digest"),
        "the refusal names the digest mismatch: {err}"
    );
}
