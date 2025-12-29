//! Unit tests for JWT token extraction
//!
//! Tests cover:
//! - Token extraction from Authorization header (Bearer token)
//! - Token extraction from cookies (access_token cookie)
//! - Handling of missing tokens
//! - Handling of malformed headers
//! - JwtExtractor creation and validation

use axum::http::{header, HeaderMap, HeaderValue};
use systemprompt_core_api::services::middleware::jwt::extract_token_from_headers;

// ============================================================================
// Authorization Header Token Extraction Tests
// ============================================================================

#[test]
fn test_extract_token_from_bearer_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test_token_12345"),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("test_token_12345".to_string()));
}

#[test]
fn test_extract_token_from_bearer_header_complex_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"),
    );

    let token = extract_token_from_headers(&headers);
    assert!(token.is_some());
    assert!(token.unwrap().starts_with("eyJ"));
}

#[test]
fn test_extract_token_missing_bearer_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("test_token_no_bearer"),
    );

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_empty_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer "));

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_bearer_only() {
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer"));

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_lowercase_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("bearer token123"),
    );

    // Should not match since "Bearer" is case-sensitive
    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_basic_auth_not_matched() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

// ============================================================================
// Cookie Token Extraction Tests
// ============================================================================

#[test]
fn test_extract_token_from_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("access_token=cookie_token_value"),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("cookie_token_value".to_string()));
}

#[test]
fn test_extract_token_from_cookie_with_multiple_cookies() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("session=abc123; access_token=my_jwt_token; user_id=456"),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("my_jwt_token".to_string()));
}

#[test]
fn test_extract_token_from_cookie_with_spaces() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("  access_token=spaced_token  "),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("spaced_token".to_string()));
}

#[test]
fn test_extract_token_empty_cookie_value() {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, HeaderValue::from_static("access_token="));

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_wrong_cookie_name() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("auth_token=wrong_name"),
    );

    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

// ============================================================================
// Priority Tests (Header vs Cookie)
// ============================================================================

#[test]
fn test_extract_token_header_takes_priority() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer header_token"),
    );
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("access_token=cookie_token"),
    );

    let token = extract_token_from_headers(&headers);
    // Header should be checked first
    assert_eq!(token, Some("header_token".to_string()));
}

#[test]
fn test_extract_token_fallback_to_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("access_token=fallback_token"),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("fallback_token".to_string()));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_extract_token_empty_headers() {
    let headers = HeaderMap::new();
    let token = extract_token_from_headers(&headers);
    assert!(token.is_none());
}

#[test]
fn test_extract_token_with_special_characters() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer token-with_special.chars+123"),
    );

    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some("token-with_special.chars+123".to_string()));
}

#[test]
fn test_extract_token_bearer_with_extra_spaces() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer  token_with_extra_space"),
    );

    // The extra space becomes part of the token
    let token = extract_token_from_headers(&headers);
    assert_eq!(token, Some(" token_with_extra_space".to_string()));
}
