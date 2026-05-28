//! Tests for `AuthenticationService` early-exit paths.
//!
//! The success path requires a populated `Config` singleton and a JWT
//! signed by the in-process authority keyring. Coverage here exercises
//! the deterministic 401 path that fires before either dependency is
//! reached: requests with no extractable bearer/cookie token.

use http::{HeaderMap, StatusCode};
use systemprompt_oauth::services::jwt::AuthenticationService;

#[test]
fn authenticate_returns_unauthorized_when_headers_empty() {
    let headers = HeaderMap::new();

    let err = AuthenticationService::authenticate(&headers)
        .expect_err("empty headers must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authenticate_returns_unauthorized_for_basic_auth_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Basic dXNlcjpwYXNz".parse().expect("parse header"),
    );

    let err = AuthenticationService::authenticate(&headers)
        .expect_err("basic auth must be rejected by the bearer extractor");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authenticate_returns_unauthorized_for_empty_bearer_value() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().expect("parse header"));

    let err = AuthenticationService::authenticate(&headers)
        .expect_err("empty bearer must be rejected");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authenticate_returns_unauthorized_for_cookie_without_access_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id=abc; csrf=xyz".parse().expect("parse header"),
    );

    let err = AuthenticationService::authenticate(&headers)
        .expect_err("cookie without access_token must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authentication_service_is_copy_and_debug() {
    let svc = AuthenticationService;
    let copied = svc;
    let debug = format!("{:?}", copied);

    assert!(debug.contains("AuthenticationService"));
}
