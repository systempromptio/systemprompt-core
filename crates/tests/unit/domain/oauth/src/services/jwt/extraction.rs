//! Tests for JWT token extraction from headers

use axum::http::{HeaderMap, StatusCode};
use systemprompt_core_oauth::services::jwt::extraction::TokenExtractor;
use systemprompt_core_oauth::{extract_bearer_token, extract_cookie_token};

// ============================================================================
// extract_bearer_token Tests
// ============================================================================

#[test]
fn test_extract_bearer_token_success() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer test_token_123".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_token_123");
}

#[test]
fn test_extract_bearer_token_missing_header() {
    let headers = HeaderMap::new();

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_wrong_scheme() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Basic dXNlcjpwYXNz".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_missing_space() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearertoken".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_empty_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());

    let result = extract_bearer_token(&headers);
    // The function strips prefix and returns what's left
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_extract_bearer_token_case_sensitive() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer token123".parse().unwrap());

    let result = extract_bearer_token(&headers);
    // "bearer" (lowercase) won't match "Bearer "
    assert!(result.is_err());
}

#[test]
fn test_extract_bearer_token_with_spaces_in_token() {
    let mut headers = HeaderMap::new();
    // Token with spaces is technically invalid but let's see what happens
    headers.insert("authorization", "Bearer token with spaces".parse().unwrap());

    let result = extract_bearer_token(&headers);
    assert!(result.is_ok());
    // Should get everything after "Bearer "
    assert_eq!(result.unwrap(), "token with spaces");
}

#[test]
fn test_extract_bearer_token_jwt_format() {
    let mut headers = HeaderMap::new();
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
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
    headers.insert("cookie", "access_token=token123".parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "token123");
}

#[test]
fn test_extract_cookie_token_multiple_cookies() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id=abc; access_token=token456; other=value".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "token456");
}

#[test]
fn test_extract_cookie_token_missing_header() {
    let headers = HeaderMap::new();

    let result = extract_cookie_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_extract_cookie_token_no_access_token() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session_id=abc; other=value".parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_err());
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
fn test_extract_cookie_token_with_whitespace() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "  session=abc ;  access_token=token789 ; other=xyz  ".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    // The implementation trims cookie entries before parsing
    assert_eq!(result.unwrap(), "token789");
}

#[test]
fn test_extract_cookie_token_first_occurrence() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "access_token=first; access_token=second".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    // Should get the first one
    assert_eq!(result.unwrap(), "first");
}

#[test]
fn test_extract_cookie_token_jwt_format() {
    let mut headers = HeaderMap::new();
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    headers.insert("cookie", format!("access_token={}", jwt).parse().unwrap());

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), jwt);
}

#[test]
fn test_extract_cookie_token_partial_name_no_match() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "my_access_token=token123".parse().unwrap());

    let result = extract_cookie_token(&headers);
    // "my_access_token" should not match "access_token"
    assert!(result.is_err());
}

#[test]
fn test_extract_cookie_token_at_end() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session=abc; other=def; access_token=last_token".parse().unwrap(),
    );

    let result = extract_cookie_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "last_token");
}

// ============================================================================
// TokenExtractor Tests
// ============================================================================

#[test]
fn test_token_extractor_bearer_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer test_token".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_token");
}

#[test]
fn test_token_extractor_mcp_proxy_header() {
    let mut headers = HeaderMap::new();
    headers.insert("x-mcp-proxy-auth", "Bearer mcp_token".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "mcp_token");
}

#[test]
fn test_token_extractor_cookie_fallback() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "access_token=cookie_token".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "cookie_token");
}

#[test]
fn test_token_extractor_priority_authorization_first() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer auth_token".parse().unwrap());
    headers.insert("x-mcp-proxy-auth", "Bearer mcp_token".parse().unwrap());
    headers.insert("cookie", "access_token=cookie_token".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "auth_token");
}

#[test]
fn test_token_extractor_priority_mcp_before_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert("x-mcp-proxy-auth", "Bearer mcp_token".parse().unwrap());
    headers.insert("cookie", "access_token=cookie_token".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "mcp_token");
}

#[test]
fn test_token_extractor_no_headers() {
    let headers = HeaderMap::new();

    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_token_extractor_empty_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());

    let result = TokenExtractor::extract_bearer_token(&headers);
    // Empty token after Bearer should fail
    assert!(result.is_err());
}

#[test]
fn test_token_extractor_mcp_no_bearer_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert("x-mcp-proxy-auth", "token_without_bearer".parse().unwrap());

    // No cookie fallback
    let result = TokenExtractor::extract_bearer_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_token_extractor_debug() {
    let extractor = TokenExtractor;
    let debug_str = format!("{:?}", extractor);
    assert!(debug_str.contains("TokenExtractor"));
}

#[test]
fn test_token_extractor_copy() {
    let extractor = TokenExtractor;
    let copied = extractor;
    let _ = copied;
    let _ = extractor; // Both should be usable
}

#[test]
fn test_token_extractor_clone() {
    let extractor = TokenExtractor;
    let cloned = extractor.clone();
    let _ = cloned;
}
