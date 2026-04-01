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

    let result = extractor.extract_from_mcp_proxy(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "mcp_token");
}

#[test]
fn test_extract_from_mcp_proxy_missing() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let result = extractor.extract_from_mcp_proxy(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::MissingMcpProxyHeader
    );
}

#[test]
fn test_extract_from_mcp_proxy_invalid_format() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-mcp-proxy-auth",
        HeaderValue::from_static("token_without_bearer"),
    );

    let result = extractor.extract_from_mcp_proxy(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::InvalidMcpProxyFormat
    );
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

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "cookie_token_value");
}

#[test]
fn test_extract_from_cookie_multiple_cookies() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("session=abc123; access_token=the_token; other=value"),
    );

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "the_token");
}

#[test]
fn test_extract_from_cookie_missing() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), TokenExtractionError::MissingCookie);
}

#[test]
fn test_extract_from_cookie_token_not_found() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("other_cookie=value"));

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_from_cookie_empty_value() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("access_token="));

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_from_cookie_with_spaces() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("  access_token=spaced_token  ; other=val"),
    );

    let result = extractor.extract_from_cookie(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "spaced_token");
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

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "auth_token");
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

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "mcp_token");
}

#[test]
fn test_extract_fallback_to_cookie() {
    let extractor = TokenExtractor::standard();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=cookie_token"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "cookie_token");
}

#[test]
fn test_extract_no_token_found() {
    let extractor = TokenExtractor::standard();
    let headers = HeaderMap::new();

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), TokenExtractionError::NoTokenFound);
}

#[test]
fn test_extract_empty_chain() {
    let extractor = TokenExtractor::new(vec![]);
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer token"));

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), TokenExtractionError::NoTokenFound);
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
