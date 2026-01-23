//! Tests for token extraction from HTTP headers

use http::HeaderMap;
use systemprompt_oauth::{extract_bearer_token, extract_cookie_token};

// ============================================================================
// extract_bearer_token Tests
// ============================================================================

#[test]
fn test_extract_bearer_token_success() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer my_test_token_12345".parse().unwrap(),
    );

    let result = extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_test_token_12345");
}

#[test]
fn test_extract_bearer_token_missing_header() {
    let headers = HeaderMap::new();

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_invalid_format_no_bearer_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "my_test_token".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_empty_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_basic_auth_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Basic dXNlcm5hbWU6cGFzc3dvcmQ=".parse().unwrap(),
    );

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_lowercase_bearer_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer my_token_123".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_with_jwt_format() {
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    let mut headers = HeaderMap::new();
    headers.insert("authorization", format!("Bearer {}", jwt).parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), jwt);
}

// ============================================================================
// extract_cookie_token Tests
// ============================================================================

#[test]
fn test_extract_cookie_token_success() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "access_token=my_token_value".parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_token_value");
}

#[test]
fn test_extract_cookie_token_missing_header() {
    let headers = HeaderMap::new();

    let result = extract_cookie_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_cookie_token_no_access_token_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session_id=abc123".parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_cookie_token_multiple_cookies() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id=abc123; access_token=my_jwt_token; other_cookie=value"
            .parse()
            .unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_jwt_token");
}

#[test]
fn test_extract_cookie_token_with_spaces() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id=abc123;  access_token=my_token;  other=val"
            .parse()
            .unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_token");
}

#[test]
fn test_extract_cookie_token_at_start() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "access_token=first_token; session_id=xyz".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "first_token");
}

#[test]
fn test_extract_cookie_token_empty_value() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "access_token=".parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_extract_cookie_token_jwt_format() {
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature";
    let mut headers = HeaderMap::new();
    headers.insert("cookie", format!("access_token={}", jwt).parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), jwt);
}

#[test]
fn test_extract_cookie_token_similar_cookie_name() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "my_access_token=wrong; access_token=correct".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "correct");
}
