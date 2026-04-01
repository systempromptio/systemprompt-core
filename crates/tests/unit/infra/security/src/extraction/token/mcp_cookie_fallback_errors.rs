//! Tests for MCP proxy extraction, cookie extraction, fallback chain, and error display

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_security::{TokenExtractionError, TokenExtractor};

// ============================================================================
// MCP Proxy Header Extraction Tests
// ============================================================================

#[test]
fn test_extract_from_mcp_proxy_success() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-mcp-proxy-auth",
        HeaderValue::from_static("Bearer mcp_token"),
    );

    let token = extractor.extract_from_mcp_proxy(&headers)
        .expect("Should extract from MCP proxy header");
    assert_eq!(token, "mcp_token");
}

#[test]
fn test_extract_from_mcp_proxy_missing() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let err = extractor.extract_from_mcp_proxy(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::MissingMcpProxyHeader);
}

#[test]
fn test_extract_from_mcp_proxy_invalid_format() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-mcp-proxy-auth",
        HeaderValue::from_static("token_without_bearer"),
    );

    let err = extractor.extract_from_mcp_proxy(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::InvalidMcpProxyFormat);
}

// ============================================================================
// Cookie Extraction Tests
// ============================================================================

#[test]
fn test_extract_from_cookie_success() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=cookie_token_value"),
    );

    let token = extractor.extract_from_cookie(&headers)
        .expect("Should extract from cookie");
    assert_eq!(token, "cookie_token_value");
}

#[test]
fn test_extract_from_cookie_multiple_cookies() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("session=abc123; access_token=the_token; other=value"),
    );

    let token = extractor.extract_from_cookie(&headers)
        .expect("Should extract from multiple cookies");
    assert_eq!(token, "the_token");
}

#[test]
fn test_extract_from_cookie_missing() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let err = extractor.extract_from_cookie(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::MissingCookie);
}

#[test]
fn test_extract_from_cookie_token_not_found() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("other_cookie=value"));

    let err = extractor.extract_from_cookie(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::TokenNotFoundInCookie);
}

#[test]
fn test_extract_from_cookie_empty_value() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("access_token="));

    let err = extractor.extract_from_cookie(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::TokenNotFoundInCookie);
}

#[test]
fn test_extract_from_cookie_with_spaces() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("  access_token=spaced_token  ; other=val"),
    );

    let token = extractor.extract_from_cookie(&headers)
        .expect("Should extract from cookie with spaces");
    assert_eq!(token, "spaced_token");
}

// ============================================================================
// Fallback Chain Tests
// ============================================================================

#[test]
fn test_extract_fallback_authorization_first() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer auth_token"),
    );
    headers.insert(
        "x-mcp-proxy-auth",
        HeaderValue::from_static("Bearer mcp_token"),
    );
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=cookie_token"),
    );

    let token = extractor.extract(&headers).expect("Should extract from authorization first");
    assert_eq!(token, "auth_token");
}

#[test]
fn test_extract_fallback_to_mcp_proxy() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-mcp-proxy-auth",
        HeaderValue::from_static("Bearer mcp_token"),
    );
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=cookie_token"),
    );

    let token = extractor.extract(&headers).expect("Should fallback to MCP proxy");
    assert_eq!(token, "mcp_token");
}

#[test]
fn test_extract_fallback_to_cookie() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=cookie_token"),
    );

    let token = extractor.extract(&headers).expect("Should fallback to cookie");
    assert_eq!(token, "cookie_token");
}

#[test]
fn test_extract_no_token_found() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let err = extractor.extract(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::NoTokenFound);
}

#[test]
fn test_extract_empty_chain() {
    let extractor = TokenExtractor::new(vec![]);
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer token"));

    let err = extractor.extract(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::NoTokenFound);
}

// ============================================================================
// TokenExtractionError Display Tests
// ============================================================================

#[test]
fn test_token_extraction_error_display_no_token_found() {
    let error = TokenExtractionError::NoTokenFound;
    assert_eq!(format!("{}", error), "No token found in request");
}

#[test]
fn test_token_extraction_error_display_missing_authorization() {
    let error = TokenExtractionError::MissingAuthorizationHeader;
    assert_eq!(format!("{}", error), "Missing Authorization header");
}

#[test]
fn test_token_extraction_error_display_invalid_authorization() {
    let error = TokenExtractionError::InvalidAuthorizationFormat;
    assert_eq!(
        format!("{}", error),
        "Invalid Authorization header format (expected 'Bearer <token>')"
    );
}

#[test]
fn test_token_extraction_error_display_missing_mcp_proxy() {
    let error = TokenExtractionError::MissingMcpProxyHeader;
    assert_eq!(
        format!("{}", error),
        "Missing MCP proxy authorization header"
    );
}

#[test]
fn test_token_extraction_error_display_invalid_mcp_proxy() {
    let error = TokenExtractionError::InvalidMcpProxyFormat;
    assert_eq!(
        format!("{}", error),
        "Invalid MCP proxy header format (expected 'Bearer <token>')"
    );
}

#[test]
fn test_token_extraction_error_display_missing_cookie() {
    let error = TokenExtractionError::MissingCookie;
    assert_eq!(format!("{}", error), "Missing cookie header");
}

#[test]
fn test_token_extraction_error_display_invalid_cookie() {
    let error = TokenExtractionError::InvalidCookieFormat;
    assert_eq!(format!("{}", error), "Invalid cookie format");
}

#[test]
fn test_token_extraction_error_display_token_not_in_cookie() {
    let error = TokenExtractionError::TokenNotFoundInCookie;
    assert_eq!(format!("{}", error), "Token not found in cookies");
}

#[test]
fn test_token_extraction_error_is_std_error() {
    let error: Box<dyn std::error::Error> = Box::new(TokenExtractionError::NoTokenFound);
    assert!(error.to_string().contains("No token found"));
}
