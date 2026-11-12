use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use bytes::Bytes;
use ipnet::IpNet;
use systemprompt_api::error::ApiHttpError;
use systemprompt_api::services::middleware::client_addr::resolve_client_ip;
use systemprompt_api::services::middleware::context::PayloadSource;
use systemprompt_api::services::middleware::context::middleware::error::extraction_error_to_api_error;
use systemprompt_api::services::middleware::{BodyReadError, read_body_bounded};
use systemprompt_api::services::session_cookie::{SameSite, SessionCookie};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_models::net::BUFFERED_BODY_LIMIT_BYTES;

fn endless_body() -> Body {
    let chunk = Bytes::from(vec![b'x'; 64 * 1024]);
    let stream = futures_util::stream::repeat_with(move || Ok::<_, Infallible>(chunk.clone()));
    Body::from_stream(stream)
}

fn declared_oversize_request() -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/agents/a/")
        .header(
            header::CONTENT_LENGTH,
            (BUFFERED_BODY_LIMIT_BYTES + 1).to_string(),
        )
        .body(Body::from_stream(futures_util::stream::pending::<
            Result<Bytes, Infallible>,
        >()))
        .expect("request builds")
}

#[tokio::test]
async fn a_body_past_the_limit_is_refused_after_reading_at_most_the_limit() {
    let error = read_body_bounded(endless_body(), 1024 * 1024)
        .await
        .expect_err("an endless body cannot be read whole");

    assert!(matches!(error, BodyReadError::TooLarge { limit } if limit == 1024 * 1024));
}

#[tokio::test]
async fn a_body_within_the_limit_is_returned_whole() {
    let bytes = read_body_bounded(Body::from("hello"), 16)
        .await
        .expect("a small body reads");

    assert_eq!(&bytes[..], b"hello");
}

#[tokio::test]
async fn an_a2a_body_past_the_limit_is_a_413_not_a_buffered_read() {
    let (parts, body) = endless_body_request().into_parts();
    let request = Request::from_parts(parts, body);

    let error = PayloadSource::read_and_reconstruct(request, BUFFERED_BODY_LIMIT_BYTES)
        .await
        .map(|_| ())
        .expect_err("an oversized A2A body is refused");

    assert!(
        matches!(error, ContextExtractionError::PayloadTooLarge { limit } if limit == BUFFERED_BODY_LIMIT_BYTES),
        "{error:?}"
    );
    let api = extraction_error_to_api_error(&error);
    assert_eq!(
        ApiHttpError::from(api).into_response().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        ApiHttpError::from(error).into_response().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn an_a2a_body_whose_declared_length_is_past_the_limit_is_refused_without_reading() {
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        PayloadSource::read_and_reconstruct(declared_oversize_request(), BUFFERED_BODY_LIMIT_BYTES),
    )
    .await
    .expect("the refusal must not wait on a body that never arrives")
    .map(|_| ())
    .expect_err("a declared oversize body is refused");

    assert!(
        matches!(error, ContextExtractionError::PayloadTooLarge { .. }),
        "{error:?}"
    );
}

fn endless_body_request() -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/agents/a/")
        .body(endless_body())
        .expect("request builds")
}

fn trusted_peer() -> ConnectInfo<SocketAddr> {
    ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 443)))
}

fn trusted() -> Vec<IpNet> {
    vec!["10.0.0.0/8".parse().expect("cidr")]
}

#[test]
fn every_forwarded_for_field_is_walked_so_an_appended_field_wins_over_a_forged_first_one() {
    let mut headers = HeaderMap::new();
    headers.append("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));
    headers.append(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.9, 10.0.0.7"),
    );

    let resolved = resolve_client_ip(&headers, Some(&trusted_peer()), &trusted());

    assert_eq!(
        resolved,
        Some("198.51.100.9".parse::<IpAddr>().expect("ip"))
    );
}

#[test]
fn a_secondary_client_header_is_ignored_when_a_forwarded_for_chain_is_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.5"));
    headers.insert("x-real-ip", HeaderValue::from_static("1.2.3.4"));

    let resolved = resolve_client_ip(&headers, Some(&trusted_peer()), &trusted());

    assert_eq!(resolved, Some("10.0.0.1".parse::<IpAddr>().expect("ip")));
}

fn https() -> SessionCookie {
    SessionCookie::for_external_url("https://app.example.com")
}

fn plain_http() -> SessionCookie {
    SessionCookie::for_external_url("http://192.168.1.20:8080")
}

#[test]
fn an_https_deployment_always_issues_a_secure_http_only_cookie() {
    for same_site in [SameSite::Strict, SameSite::Lax] {
        let cookie = https().issue("tok", 60, same_site).expect("cookie builds");
        let text = cookie.to_str().expect("ascii");
        assert!(text.starts_with("access_token=tok;"), "{text}");
        for attr in ["Path=/", "HttpOnly", "Secure", "Max-Age=60"] {
            assert!(text.contains(attr), "{attr} missing from {text}");
        }
    }
    let cleared = https().cleared();
    let text = cleared.to_str().expect("ascii");
    for attr in [
        "access_token=;",
        "Path=/",
        "HttpOnly",
        "Secure",
        "Max-Age=0",
    ] {
        assert!(text.contains(attr), "{attr} missing from {text}");
    }
}

#[test]
fn a_plain_http_deployment_issues_a_cookie_the_browser_will_store() {
    let cookie = plain_http()
        .issue("tok", 60, SameSite::Lax)
        .expect("cookie builds");
    let text = cookie.to_str().expect("ascii");
    assert!(!text.contains("Secure"), "{text}");
    assert!(text.contains("HttpOnly"), "{text}");
    assert!(
        !plain_http()
            .cleared()
            .to_str()
            .expect("ascii")
            .contains("Secure")
    );
}

#[test]
fn an_unparseable_external_url_issues_a_secure_cookie() {
    assert!(SessionCookie::for_external_url("not a url").is_secure());
}

#[test]
fn the_cookie_follows_the_configured_external_url_not_the_request() {
    let mut config = systemprompt_test_fixtures::fixture_config("postgres://localhost/cookie");
    config.api_external_url = "https://gateway.example.com".to_owned();
    assert!(SessionCookie::from_config(&config).is_secure());
    config.api_external_url = "http://gateway.internal:8080".to_owned();
    assert!(!SessionCookie::from_config(&config).is_secure());
}
