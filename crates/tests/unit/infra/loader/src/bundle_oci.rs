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
async fn an_unparseable_manifest_is_refused_before_any_blob_is_requested() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not an OCI manifest"))
        .mount(&server)
        .await;

    let error = fetcher(&server)
        .head()
        .await
        .expect_err("invalid manifest must not yield a remote reference");

    assert!(
        error.to_string().contains("manifest does not parse"),
        "{error}"
    );
}

#[tokio::test]
async fn a_manifest_layer_over_the_configured_limit_is_refused_before_download() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    let dir = tempfile::tempdir().unwrap();

    let error = fetcher(&server)
        .with_max_bytes(4)
        .fetch(&dir.path().join("bundle.tar.gz"))
        .await
        .expect_err("declared layer size must be bounded before streaming");

    assert!(
        error.to_string().contains("exceeds the 4 byte limit"),
        "{error}"
    );
    assert!(!dir.path().join("bundle.tar.gz").exists());
}

#[tokio::test]
async fn a_blob_redirect_loop_is_refused_after_the_bounded_hop_limit() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/org/bundle/blobs/{}", digest_of(BODY))))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/loop"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/loop"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/loop"))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();

    let error = fetcher(&server)
        .fetch(&dir.path().join("bundle.tar.gz"))
        .await
        .expect_err("redirect loop must not keep the fetch alive");

    assert!(
        error.to_string().contains("redirected more than 3 times"),
        "{error}"
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

#[tokio::test]
async fn a_blob_redirect_does_not_forward_the_registry_credential_to_storage() {
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/archive"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&storage)
        .await;
    let registry = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest_body(one_layer(BODY))))
        .mount(&registry)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/org/bundle/blobs/{}", digest_of(BODY))))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", &format!("{}/archive", storage.uri())),
        )
        .mount(&registry)
        .await;
    let host = registry.uri().replace("http://", "");
    let fetcher = OciFetcher::new(
        "base",
        &format!("{host}/org/bundle:v1"),
        Some("registry-token".to_owned()),
        client(),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();

    let fetched = fetcher
        .fetch(&dir.path().join("bundle.tar.gz"))
        .await
        .unwrap();
    let storage_requests = storage.received_requests().await.unwrap();

    assert_eq!(std::fs::read(fetched.archive).unwrap(), BODY);
    assert_eq!(storage_requests.len(), 1);
    assert!(!storage_requests[0].headers.contains_key("authorization"));
}
#[tokio::test]
async fn manifest_outage_writes_nothing_and_a_later_pull_recovers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .respond_with(ResponseTemplate::new(503))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_manifest(&server, one_layer(BODY)).await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/org/bundle/blobs/{}", digest_of(BODY))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("owned output directory");
    let output = dir.path().join("bundle.tar.gz");

    let error = fetcher(&server)
        .fetch(&output)
        .await
        .expect_err("registry outage must fail the pull");
    assert!(error.to_string().contains("manifest request failed: 503"));
    assert!(
        !output.exists(),
        "manifest failure must not create an archive"
    );

    let fetched = fetcher(&server)
        .fetch(&output)
        .await
        .expect("the next pull recovers");
    assert_eq!(fetched.archive, output);
    assert_eq!(fetched.digest, digest_of(BODY));
    assert_eq!(std::fs::read(&output).unwrap(), BODY);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/v2/org/bundle/manifests/v1")
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path().contains("/blobs/"))
            .count(),
        1
    );
}

#[tokio::test]
async fn blob_outage_preserves_no_archive_and_a_later_pull_recovers() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    let blob_path = format!("/v2/org/bundle/blobs/{}", digest_of(BODY));
    Mock::given(method("GET"))
        .and(path(blob_path.clone()))
        .respond_with(ResponseTemplate::new(502))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(blob_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .with_priority(10)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("owned output directory");
    let output = dir.path().join("bundle.tar.gz");

    let error = fetcher(&server)
        .fetch(&output)
        .await
        .expect_err("blob outage must fail the pull");
    assert!(error.to_string().contains("blob request failed: 502"));
    assert!(
        !output.exists(),
        "failed response must not create an archive"
    );

    let fetched = fetcher(&server)
        .fetch(&output)
        .await
        .expect("the next pull recovers");
    assert_eq!(fetched.digest, digest_of(BODY));
    assert_eq!(std::fs::read(&output).unwrap(), BODY);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/v2/org/bundle/manifests/v1")
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path().contains("/blobs/"))
            .count(),
        2
    );
}

#[tokio::test]
async fn redirect_without_location_writes_nothing_and_direct_retry_recovers() {
    let server = MockServer::start().await;
    mount_manifest(&server, one_layer(BODY)).await;
    let blob_path = format!("/v2/org/bundle/blobs/{}", digest_of(BODY));
    Mock::given(method("GET"))
        .and(path(blob_path.clone()))
        .respond_with(ResponseTemplate::new(307))
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(blob_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .with_priority(10)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("owned output directory");
    let output = dir.path().join("bundle.tar.gz");

    let error = fetcher(&server)
        .fetch(&output)
        .await
        .expect_err("redirect without Location must be refused");
    assert!(
        error
            .to_string()
            .contains("redirect (307 Temporary Redirect) without a Location")
    );
    assert!(!output.exists());

    let fetched = fetcher(&server)
        .fetch(&output)
        .await
        .expect("direct blob retry recovers");
    assert_eq!(fetched.digest, digest_of(BODY));
    assert_eq!(std::fs::read(&output).unwrap(), BODY);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/v2/org/bundle/manifests/v1")
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path().contains("/blobs/"))
            .count(),
        2
    );
}
