//! Choosing a transport from a profile source, and the registry client's own
//! URL and reference handling.
//!
//! A source that declares both transports, or neither, is a configuration
//! error: guessing one would make which bytes an instance runs depend on
//! field order.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use systemprompt_loader::bundle::BundleFetcher;
use systemprompt_loader::bundle::source::oci::RegistryClient;
use systemprompt_loader::bundle::source::{AnyFetcher, RemoteRef};
use systemprompt_models::profile::{
    BundleVerification, HttpsServicesSource, OciServicesSource, ServicesSource,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client")
}

fn source(https: Option<&str>, oci: Option<&str>) -> ServicesSource {
    ServicesSource {
        name: "base".to_owned(),
        https: https.map(|url| HttpsServicesSource {
            url: url.to_owned(),
            auth_secret: None,
            verify: BundleVerification::default(),
        }),
        oci: oci.map(|reference| OciServicesSource {
            reference: reference.to_owned(),
            auth_secret: None,
            verify: BundleVerification::default(),
        }),
    }
}

#[test]
fn an_https_source_selects_the_https_transport() {
    let fetcher = AnyFetcher::from_source(
        &source(Some("https://example.com/b.tar.gz"), None),
        None,
        &client(),
    )
    .expect("fetcher");

    assert!(matches!(fetcher, AnyFetcher::Https(_)));
}

#[test]
fn an_oci_source_selects_the_registry_transport() {
    let fetcher =
        AnyFetcher::from_source(&source(None, Some("reg.example/o/b:v1")), None, &client())
            .expect("fetcher");

    assert!(matches!(fetcher, AnyFetcher::Oci(_)));
}

#[test]
fn a_source_declaring_both_transports_is_refused() {
    let err = AnyFetcher::from_source(
        &source(Some("https://example.com/b.tar.gz"), Some("reg/o/b:v1")),
        None,
        &client(),
    )
    .expect_err("two transports are ambiguous");

    assert!(
        err.to_string().contains("exactly one"),
        "the error names the ambiguity: {err}"
    );
}

#[test]
fn a_source_declaring_no_transport_is_refused() {
    let err = AnyFetcher::from_source(&source(None, None), None, &client())
        .expect_err("no transport is no source");

    assert!(
        err.to_string().contains("exactly one"),
        "the error names the missing transport: {err}"
    );
}

#[test]
fn an_oci_source_whose_reference_does_not_parse_is_refused() {
    let err = AnyFetcher::from_source(&source(None, Some("")), None, &client())
        .expect_err("an empty reference names no registry");

    assert!(
        err.to_string().contains("base"),
        "the error names the source: {err}"
    );
}

#[tokio::test]
async fn the_dispatching_fetcher_forwards_head_and_fetch_to_the_registry() {
    let server = MockServer::start().await;
    let body = b"bundle-bytes".as_slice();
    let digest = format!(
        "sha256:{}",
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(body))
    );
    Mock::given(method("GET"))
        .and(path("/v2/o/b/manifests/v1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("docker-content-digest", "sha256:head")
                .set_body_json(json!({
                    "schemaVersion": 2,
                    "config": {"mediaType": "application/json", "digest": "sha256:0", "size": 0},
                    "layers": [{
                        "mediaType": systemprompt_models::services::bundle::BUNDLE_MEDIA_TYPE,
                        "digest": digest,
                        "size": body.len()
                    }]
                })),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/o/b/blobs/{digest}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;
    let host = server.uri().replace("http://", "");
    let fetcher = AnyFetcher::from_source(
        &source(None, Some(&format!("{host}/o/b:v1"))),
        None,
        &client(),
    )
    .expect("fetcher");
    let dir = tempfile::tempdir().expect("tempdir");

    assert_eq!(fetcher.head().await.expect("head").digest, "sha256:head");
    let fetched = fetcher
        .fetch(&dir.path().join("b.tar.gz"))
        .await
        .expect("fetch");
    assert_eq!(std::fs::read(&fetched.archive).expect("read"), body);
}

#[test]
fn an_empty_digest_reads_as_an_unknown_remote() {
    assert!(
        RemoteRef {
            digest: String::new()
        }
        .is_unknown()
    );
    assert!(
        !RemoteRef {
            digest: "sha256:abc".to_owned()
        }
        .is_unknown()
    );
}

#[test]
fn a_reference_without_a_tag_or_digest_addresses_latest() {
    let registry =
        RegistryClient::new("base", "reg.example/o/b", None, client()).expect("registry");

    assert_eq!(registry.manifest_ref(), "latest");
}

#[test]
fn a_digest_reference_wins_over_a_tag() {
    let registry = RegistryClient::new(
        "base",
        "reg.example/o/b:v1@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        client(),
    )
    .expect("registry");

    assert_eq!(
        registry.manifest_ref(),
        format!("sha256:{}", "a".repeat(64))
    );
}

#[test]
fn a_non_loopback_registry_is_addressed_over_https() {
    let registry =
        RegistryClient::new("base", "reg.example/o/b:v1", None, client()).expect("registry");

    assert_eq!(
        registry.url("/manifests/v1").expect("url").as_str(),
        "https://reg.example/v2/o/b/manifests/v1"
    );
}

#[test]
fn a_loopback_registry_is_addressed_over_plain_http() {
    let registry =
        RegistryClient::new("base", "localhost:5000/o/b:v1", None, client()).expect("registry");

    assert_eq!(
        registry.url("/manifests/v1").expect("url").as_str(),
        "http://localhost:5000/v2/o/b/manifests/v1"
    );
}
