//! The HTTPS bundle transport against a stub origin.
//!
//! The origin is plain HTTP on loopback, which the SSRF guard permits; the
//! rejection test uses a non-loopback host to prove the guard is still in the
//! path rather than bypassed for convenience.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_loader::bundle::BundleFetcher;
use systemprompt_loader::bundle::source::https::HttpsFetcher;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client")
}

const BODY: &[u8] = b"not-a-real-archive-but-bytes-are-bytes";

async fn origin_with(etag: Option<&str>) -> MockServer {
    let server = MockServer::start().await;
    let get = etag.map_or_else(
        || ResponseTemplate::new(200).set_body_bytes(BODY),
        |tag| {
            ResponseTemplate::new(200)
                .set_body_bytes(BODY)
                .insert_header("etag", tag)
        },
    );
    let head = etag.map_or_else(
        || ResponseTemplate::new(200),
        |tag| ResponseTemplate::new(200).insert_header("etag", tag),
    );
    Mock::given(method("GET"))
        .and(path("/bundle.tar.gz"))
        .respond_with(get)
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/bundle.tar.gz"))
        .respond_with(head)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn a_bundle_downloads_and_reports_its_digest() {
    let server = origin_with(Some("v1")).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        None,
        client(),
    );

    let archive = dir.path().join("b.tar.gz");
    let fetched = fetcher.fetch(&archive).await.expect("fetch");

    assert_eq!(
        std::fs::read(&fetched.archive).expect("read"),
        BODY,
        "the streamed body lands intact"
    );
    assert_eq!(
        fetched.digest,
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(BODY)),
        "the digest is over the received bytes"
    );
}

#[tokio::test]
async fn an_unchanged_etag_is_reported_by_head_without_downloading() {
    let server = origin_with(Some("v1")).await;
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        None,
        client(),
    );

    let first = fetcher.head().await.expect("head");
    let second = fetcher.head().await.expect("head again");

    assert_eq!(first.digest, "v1");
    assert_eq!(first, second, "a stable ETag is the no-change signal");
    let gets = server
        .received_requests()
        .await
        .expect("requests")
        .iter()
        .filter(|r: &&Request| r.method == wiremock::http::Method::GET)
        .count();
    assert_eq!(gets, 0, "head must not download the body");
}

#[tokio::test]
async fn an_origin_without_an_etag_reports_an_unknown_digest() {
    let server = origin_with(None).await;
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        None,
        client(),
    );

    let remote = fetcher.head().await.expect("head");

    assert!(
        remote.is_unknown(),
        "no ETag must read as unknown, never as unchanged"
    );
}

#[tokio::test]
async fn a_bearer_token_is_sent_when_the_source_names_an_auth_secret() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bundle.tar.gz"))
        .and(header("authorization", "Bearer s3cret"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        Some("s3cret".to_owned()),
        client(),
    );

    fetcher
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect("the authorised request should be served");
}

#[tokio::test]
async fn a_redirect_is_refused_rather_than_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bundle.tar.gz"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "https://evil.test/x"))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().expect("tempdir");
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        None,
        client(),
    );

    let err = fetcher
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect_err("a redirect must not be followed");

    assert!(
        err.to_string().contains("redirect"),
        "the refusal names the redirect: {err}"
    );
}

#[tokio::test]
async fn a_body_over_the_cap_is_refused() {
    let server = origin_with(Some("v1")).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let fetcher = HttpsFetcher::new(
        "base",
        &format!("{}/bundle.tar.gz", server.uri()),
        None,
        client(),
    )
    .with_max_bytes(4);

    let err = fetcher
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect_err("the cap must hold");

    assert!(err.to_string().contains("byte limit"), "{err}");
}

#[tokio::test]
async fn a_plain_http_origin_that_is_not_loopback_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fetcher = HttpsFetcher::new(
        "base",
        "http://169.254.169.254/bundle.tar.gz",
        None,
        client(),
    );

    let err = fetcher
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect_err("the SSRF guard must refuse this");

    assert!(
        !dir.path().join("b.tar.gz").exists(),
        "a refused URL must not create a file"
    );
    assert!(err.to_string().contains("base"), "{err}");
}
