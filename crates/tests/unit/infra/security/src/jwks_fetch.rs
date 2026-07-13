//! Behavioural tests for [`JwksClient`] fetch and resolution.
//!
//! A `wiremock` server stands in for the issuer's JWKS endpoint (HTTP, enabled
//! via the `test-jwks-insecure-scheme` feature). The tests assert the real
//! resolution outcomes: a matching `kid` is fetched and returned, an absent
//! `kid` yields [`JwksClientError::KeyNotFound`], a non-success status yields
//! [`JwksClientError::Status`], a malformed body yields
//! [`JwksClientError::Decode`], `fetch_at` honours an explicit JWKS URI, and
//! the host allowlist / scheme guard reject untrusted issuers.

use std::time::Duration;

use systemprompt_security::keys::jwks_client::{
    MAX_CACHE_TTL, MIN_CACHE_TTL, clamp_ttl, parse_max_age,
};
use systemprompt_security::keys::{Jwk, Jwks, JwksClient, JwksClientError, RsaSigningKey};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_jwk(kid: &str) -> Jwk {
    let signing = RsaSigningKey::generate_bits(2048).expect("generate rsa");
    Jwk::from_rsa_public_key(signing.public_key(), kid.to_owned())
}

fn host_of(uri: &str) -> String {
    url::Url::parse(uri)
        .expect("parse uri")
        .host_str()
        .expect("host")
        .to_owned()
}

async fn mount_jwks(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn fetch_resolves_matching_kid() {
    let server = MockServer::start().await;
    let jwk = test_jwk("kid-1");
    mount_jwks(
        &server,
        ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![jwk.clone()],
        }),
    )
    .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let resolved = client
        .fetch(&server.uri(), "kid-1")
        .await
        .expect("fetch matching kid");
    assert_eq!(resolved.kid, "kid-1");
    assert_eq!(resolved.n, jwk.n);
    assert_eq!(resolved.e, jwk.e);
}

#[tokio::test]
async fn fetch_missing_kid_returns_key_not_found() {
    let server = MockServer::start().await;
    mount_jwks(
        &server,
        ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![test_jwk("present")],
        }),
    )
    .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let err = client
        .fetch(&server.uri(), "absent")
        .await
        .expect_err("absent kid");
    match err {
        JwksClientError::KeyNotFound { kid, .. } => assert_eq!(kid, "absent"),
        other => panic!("expected KeyNotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_non_success_status_is_surfaced() {
    let server = MockServer::start().await;
    mount_jwks(&server, ResponseTemplate::new(503)).await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let err = client
        .fetch(&server.uri(), "kid")
        .await
        .expect_err("503 response");
    match err {
        JwksClientError::Status { status, .. } => assert_eq!(status, 503),
        other => panic!("expected Status, got {other:?}"),
    }
}

#[tokio::test]
async fn fetch_malformed_body_is_a_decode_error() {
    let server = MockServer::start().await;
    mount_jwks(
        &server,
        ResponseTemplate::new(200).set_body_string("not json"),
    )
    .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let err = client
        .fetch(&server.uri(), "kid")
        .await
        .expect_err("malformed body");
    assert!(
        matches!(err, JwksClientError::Decode { .. }),
        "expected Decode, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_at_uses_explicit_jwks_uri() {
    let server = MockServer::start().await;
    let jwk = test_jwk("explicit");
    Mock::given(method("GET"))
        .and(path("/custom/keys.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![jwk.clone()],
        }))
        .mount(&server)
        .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let jwks_uri = format!("{}/custom/keys.json", server.uri());
    let resolved = client
        .fetch_at("https://issuer.example", &jwks_uri, "explicit")
        .await
        .expect("fetch_at explicit uri");
    assert_eq!(resolved.kid, "explicit");
}

#[tokio::test]
async fn fetch_rejects_host_not_in_allowlist() {
    let server = MockServer::start().await;
    mount_jwks(
        &server,
        ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![test_jwk("kid")],
        }),
    )
    .await;

    let client = JwksClient::new(vec!["trusted.example".to_owned()]);
    let err = client
        .fetch(&server.uri(), "kid")
        .await
        .expect_err("untrusted host");
    assert!(
        matches!(err, JwksClientError::HostNotAllowed(_)),
        "expected HostNotAllowed, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_rejects_unparseable_issuer() {
    let client = JwksClient::new(vec!["trusted.example".to_owned()]);
    let err = client
        .fetch("not a url", "kid")
        .await
        .expect_err("bad issuer");
    assert!(
        matches!(err, JwksClientError::InvalidIssuer { .. }),
        "expected InvalidIssuer, got {err:?}"
    );
}

#[tokio::test]
async fn second_fetch_is_served_from_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![test_jwk("cached")],
        }))
        .expect(1)
        .mount(&server)
        .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    client
        .fetch(&server.uri(), "cached")
        .await
        .expect("first fetch");
    client
        .fetch(&server.uri(), "cached")
        .await
        .expect("second fetch served from cache");
    // `expect(1)` on the mock asserts the upstream was hit exactly once.
}

#[tokio::test]
async fn fetch_at_second_call_is_served_from_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/custom/keys.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![test_jwk("cached")],
        }))
        .expect(1)
        .mount(&server)
        .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let jwks_uri = format!("{}/custom/keys.json", server.uri());
    client
        .fetch_at("https://issuer.example", &jwks_uri, "cached")
        .await
        .expect("first fetch_at");
    let resolved = client
        .fetch_at("https://issuer.example", &jwks_uri, "cached")
        .await
        .expect("second fetch_at served from cache");
    assert_eq!(resolved.kid, "cached");
    // `expect(1)` proves the explicit-URI path also short-circuits on a cache
    // hit rather than re-fetching.
}

#[tokio::test]
async fn fetch_at_throttles_repeated_unknown_kid_lookups() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/custom/keys.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Jwks {
            keys: vec![test_jwk("present")],
        }))
        .expect(1)
        .mount(&server)
        .await;

    let client = JwksClient::new(vec![host_of(&server.uri())]);
    let jwks_uri = format!("{}/custom/keys.json", server.uri());

    let first = client
        .fetch_at("https://issuer.example", &jwks_uri, "absent")
        .await
        .expect_err("absent kid on first fetch_at");
    assert!(matches!(first, JwksClientError::KeyNotFound { .. }));

    let second = client
        .fetch_at("https://issuer.example", &jwks_uri, "absent")
        .await
        .expect_err("absent kid within the refetch throttle window");
    assert!(matches!(second, JwksClientError::KeyNotFound { .. }));
    // `expect(1)` proves the second unknown-kid lookup was throttled: the
    // upstream was not re-fetched inside the min-refresh interval.
}

#[tokio::test]
async fn fetch_rejects_non_http_scheme() {
    let client = JwksClient::new(vec!["trusted.example".to_owned()]);
    let err = client
        .fetch("ftp://trusted.example/keys", "kid")
        .await
        .expect_err("ftp scheme");
    assert!(
        matches!(err, JwksClientError::InsecureScheme(_)),
        "expected InsecureScheme, got {err:?}"
    );
}

#[test]
fn debug_impl_lists_allowed_hosts_only() {
    let client = JwksClient::new(vec!["trusted.example".to_owned()]);
    let rendered = format!("{client:?}");
    assert!(rendered.contains("JwksClient"));
    assert!(rendered.contains("trusted.example"));
}

#[test]
fn with_http_client_replaces_the_transport() {
    let client = JwksClient::new(vec!["trusted.example".to_owned()])
        .with_http_client(reqwest::Client::new());
    let rendered = format!("{client:?}");
    assert!(
        rendered.contains("trusted.example"),
        "swapping the transport preserves the allowlist"
    );
}

#[test]
fn parse_max_age_extracts_seconds_or_none() {
    assert_eq!(
        parse_max_age("public, max-age=60"),
        Some(Duration::from_secs(60))
    );
    assert_eq!(parse_max_age("no-store, no-cache"), None);
}

#[test]
fn clamp_ttl_bounds_to_the_configured_window() {
    assert_eq!(clamp_ttl(Duration::from_secs(1)), MIN_CACHE_TTL);
    assert_eq!(clamp_ttl(Duration::from_secs(100_000)), MAX_CACHE_TTL);
    let mid = Duration::from_secs(120);
    assert_eq!(clamp_ttl(mid), mid);
}
