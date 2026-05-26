//! Unit tests for the bearer-token extraction helper.
//!
//! Targets:
//! - crates/domain/agent/src/services/a2a_server/auth/validation.rs

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_agent::services::a2a_server::auth::extract_bearer_token;

#[test]
fn extract_bearer_token_present() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer abc123"));

    let token = extract_bearer_token(&headers);
    assert_eq!(token.as_deref(), Some("abc123"));
}

#[test]
fn extract_bearer_token_missing_header() {
    let headers = HeaderMap::new();
    assert!(extract_bearer_token(&headers).is_none());
}

#[test]
fn extract_bearer_token_wrong_scheme_returns_none() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );
    assert!(extract_bearer_token(&headers).is_none());
}

#[test]
fn extract_bearer_token_empty_token_returns_empty_string() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer "));
    let token = extract_bearer_token(&headers);
    assert_eq!(token.as_deref(), Some(""));
}

#[test]
fn extract_bearer_token_with_complex_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer eyJ.foo-bar_baz.signature=="),
    );

    let token = extract_bearer_token(&headers);
    assert_eq!(token.as_deref(), Some("eyJ.foo-bar_baz.signature=="));
}

#[test]
fn extract_bearer_token_lowercase_bearer_returns_none() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("bearer abc"));
    assert!(extract_bearer_token(&headers).is_none());
}
