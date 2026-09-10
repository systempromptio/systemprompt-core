//! Publishing a bundle to a stub OCI registry.
//!
//! A push is only "done" when the registry accepted the config blob, the
//! layer blob and the manifest. Each of those can be refused independently,
//! and every refusal has to surface as an error rather than a digest.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use serde_json::json;
use systemprompt_loader::bundle::source::oci::push::push_bundle;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CONFIG: &[u8] = b"{\"manifest\":true}";
const ARCHIVE: &[u8] = b"tarball-bytes";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client")
}

fn archive(dir: &Path) -> std::path::PathBuf {
    let file = dir.join("bundle.tar.gz");
    std::fs::write(&file, ARCHIVE).expect("write archive");
    file
}

fn reference(server: &MockServer) -> String {
    format!("{}/org/bundle:v1", server.uri().replace("http://", ""))
}

async fn mount_upload_session(server: &MockServer, location: &str) {
    Mock::given(method("POST"))
        .and(path("/v2/org/bundle/blobs/uploads/"))
        .respond_with(ResponseTemplate::new(202).insert_header("location", location))
        .mount(server)
        .await;
}

async fn mount_blob_put(server: &MockServer, status: u16) {
    Mock::given(method("PUT"))
        .and(path("/upload/session"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mount_manifest_put(server: &MockServer, status: u16) {
    Mock::given(method("PUT"))
        .and(path("/v2/org/bundle/manifests/v1"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn put_bodies(server: &MockServer, matcher: &str) -> Vec<Vec<u8>> {
    server
        .received_requests()
        .await
        .expect("requests")
        .into_iter()
        .filter(|r: &Request| r.method == wiremock::http::Method::PUT && r.url.path() == matcher)
        .map(|r| r.body)
        .collect()
}

#[tokio::test]
async fn a_push_uploads_both_blobs_then_the_manifest_and_returns_its_digest() {
    let server = MockServer::start().await;
    mount_upload_session(&server, "/upload/session").await;
    mount_blob_put(&server, 201).await;
    mount_manifest_put(&server, 201).await;
    let dir = tempfile::tempdir().expect("tempdir");

    let digest = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect("push");

    let blobs = put_bodies(&server, "/upload/session").await;
    assert_eq!(
        blobs,
        vec![CONFIG.to_vec(), ARCHIVE.to_vec()],
        "the config blob is uploaded before the layer"
    );
    let manifests = put_bodies(&server, "/v2/org/bundle/manifests/v1").await;
    let sent: serde_json::Value =
        serde_json::from_slice(manifests.first().expect("manifest put")).expect("manifest json");
    assert_eq!(sent["layers"].as_array().expect("layers").len(), 1);
    assert_eq!(
        sent["layers"][0]["size"].as_u64(),
        Some(ARCHIVE.len() as u64)
    );
    assert_eq!(
        digest,
        format!(
            "sha256:{}",
            hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
                manifests.first().expect("manifest put")
            ))
        ),
        "the returned digest addresses the manifest bytes that were sent"
    );
}

#[tokio::test]
async fn an_absolute_upload_location_is_honoured_verbatim() {
    let server = MockServer::start().await;
    mount_upload_session(&server, &format!("{}/upload/session", server.uri())).await;
    mount_blob_put(&server, 201).await;
    mount_manifest_put(&server, 201).await;
    let dir = tempfile::tempdir().expect("tempdir");

    push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect("push");

    assert_eq!(put_bodies(&server, "/upload/session").await.len(), 2);
}

#[tokio::test]
async fn the_blob_digest_is_sent_as_a_query_parameter() {
    let server = MockServer::start().await;
    mount_upload_session(&server, "/upload/session").await;
    let expected = format!(
        "sha256:{}",
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(CONFIG))
    );
    Mock::given(method("PUT"))
        .and(path("/upload/session"))
        .and(query_param("digest", expected.as_str()))
        .respond_with(ResponseTemplate::new(201))
        .expect(1..)
        .mount(&server)
        .await;
    mount_blob_put(&server, 201).await;
    mount_manifest_put(&server, 201).await;
    let dir = tempfile::tempdir().expect("tempdir");

    push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect("push");
}

#[tokio::test]
async fn a_refused_upload_session_stops_the_push() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/org/bundle/blobs/uploads/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("a refused session must not report a digest");

    assert!(
        err.to_string().contains("upload session refused"),
        "the error names the refused session: {err}"
    );
}

#[tokio::test]
async fn an_upload_session_without_a_location_header_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/org/bundle/blobs/uploads/"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("there is nowhere to upload to");

    assert!(
        err.to_string().contains("no Location"),
        "the error names the missing header: {err}"
    );
}

#[tokio::test]
async fn an_unparseable_upload_location_is_an_error() {
    let server = MockServer::start().await;
    mount_upload_session(&server, "http://%%%/nope").await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("a malformed location must not be guessed at");

    assert!(
        err.to_string().contains("bad upload location"),
        "the error names the bad location: {err}"
    );
}

#[tokio::test]
async fn a_rejected_blob_upload_stops_the_push_before_the_manifest() {
    let server = MockServer::start().await;
    mount_upload_session(&server, "/upload/session").await;
    mount_blob_put(&server, 413).await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("a rejected blob must not be followed by a manifest");

    assert!(
        err.to_string().contains("blob upload failed"),
        "the error names the failed upload: {err}"
    );
    assert!(
        put_bodies(&server, "/v2/org/bundle/manifests/v1")
            .await
            .is_empty(),
        "no manifest may point at bytes the registry refused"
    );
}

#[tokio::test]
async fn a_rejected_manifest_put_is_an_error_rather_than_a_digest() {
    let server = MockServer::start().await;
    mount_upload_session(&server, "/upload/session").await;
    mount_blob_put(&server, 201).await;
    mount_manifest_put(&server, 400).await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("a refused manifest is a failed publish");

    assert!(
        err.to_string().contains("manifest push failed"),
        "the error names the failed manifest: {err}"
    );
}

#[tokio::test]
async fn a_push_answers_a_bearer_challenge_and_retries() {
    let server = MockServer::start().await;
    let realm = format!("{}/token", server.uri());
    Mock::given(method("POST"))
        .and(path("/v2/org/bundle/blobs/uploads/"))
        .and(wiremock::matchers::header("authorization", "Bearer minted"))
        .respond_with(ResponseTemplate::new(202).insert_header("location", "/upload/session"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/org/bundle/blobs/uploads/"))
        .respond_with(ResponseTemplate::new(401).insert_header(
            "www-authenticate",
            format!("Bearer realm=\"{realm}\",service=\"reg\"").as_str(),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "minted"})))
        .mount(&server)
        .await;
    mount_blob_put(&server, 201).await;
    mount_manifest_put(&server, 201).await;
    let dir = tempfile::tempdir().expect("tempdir");

    push_bundle(
        &reference(&server),
        &archive(dir.path()),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect("push after challenge");

    assert_eq!(put_bodies(&server, "/upload/session").await.len(), 2);
}

#[tokio::test]
async fn a_reference_that_does_not_parse_is_refused_before_any_request() {
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle("", &archive(dir.path()), CONFIG, None, client())
        .await
        .expect_err("an empty reference names no registry");

    assert!(
        err.to_string().contains("publish"),
        "the error names the source: {err}"
    );
}

#[tokio::test]
async fn a_missing_archive_is_reported_rather_than_pushed_empty() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().expect("tempdir");

    let err = push_bundle(
        &reference(&server),
        &dir.path().join("absent.tar.gz"),
        CONFIG,
        None,
        client(),
    )
    .await
    .expect_err("a missing archive cannot be published");

    assert!(
        err.to_string().to_lowercase().contains("no such file"),
        "the error names the missing archive: {err}"
    );
}
