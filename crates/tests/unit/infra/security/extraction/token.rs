//! Unit tests for TokenExtractor and ExtractionMethod
//!
//! Tests cover:
//! - Token extraction from Authorization header
//! - Token extraction from MCP proxy header
//! - Token extraction from cookies
//! - Fallback chain behavior
//! - Error handling for missing/invalid tokens
//! - Custom configuration options

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_core_security::{ExtractionMethod, TokenExtractionError, TokenExtractor};

// ============================================================================
// ExtractionMethod Display Tests
// ============================================================================

#[test]
fn test_extraction_method_display_authorization_header() {
    let method = ExtractionMethod::AuthorizationHeader;
    assert_eq!(format!("{}", method), "Authorization header");
}

#[test]
fn test_extraction_method_display_mcp_proxy_header() {
    let method = ExtractionMethod::McpProxyHeader;
    assert_eq!(format!("{}", method), "MCP proxy header");
}

#[test]
fn test_extraction_method_display_cookie() {
    let method = ExtractionMethod::Cookie;
    assert_eq!(format!("{}", method), "Cookie");
}

// ============================================================================
// TokenExtractor Constructor Tests
// ============================================================================

#[test]
fn test_token_extractor_new_with_custom_chain() {
    let extractor = TokenExtractor::new(vec![ExtractionMethod::Cookie]);
    assert_eq!(extractor.chain().len(), 1);
    assert_eq!(extractor.chain()[0], ExtractionMethod::Cookie);
}

#[test]
fn test_token_extractor_standard() {
    let extractor = TokenExtractor::standard();
    let chain = extractor.chain();
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0], ExtractionMethod::AuthorizationHeader);
    assert_eq!(chain[1], ExtractionMethod::McpProxyHeader);
    assert_eq!(chain[2], ExtractionMethod::Cookie);
}

#[test]
fn test_token_extractor_browser_only() {
    let extractor = TokenExtractor::browser_only();
    let chain = extractor.chain();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0], ExtractionMethod::AuthorizationHeader);
    assert_eq!(chain[1], ExtractionMethod::Cookie);
}

#[test]
fn test_token_extractor_api_only() {
    let extractor = TokenExtractor::api_only();
    let chain = extractor.chain();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], ExtractionMethod::AuthorizationHeader);
}

#[test]
fn test_token_extractor_with_cookie_name() {
    let extractor = TokenExtractor::new(vec![ExtractionMethod::Cookie])
        .with_cookie_name("custom_token".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("custom_token=my_token_value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_token_value");
}

#[test]
fn test_token_extractor_with_mcp_header_name() {
    let extractor = TokenExtractor::new(vec![ExtractionMethod::McpProxyHeader])
        .with_mcp_header_name("x-custom-auth".to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-custom-auth",
        HeaderValue::from_static("Bearer custom_token"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "custom_token");
}

// ============================================================================
// Authorization Header Extraction Tests
// ============================================================================

#[test]
fn test_extract_from_authorization_header_success() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer test_token_123"),
    );

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_token_123");
}

#[test]
fn test_extract_from_authorization_header_missing() {
    let headers = HeaderMap::new();

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::MissingAuthorizationHeader
    );
}

#[test]
fn test_extract_from_authorization_header_invalid_format_no_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::InvalidAuthorizationFormat
    );
}

#[test]
fn test_extract_from_authorization_header_empty_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer "));

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::InvalidAuthorizationFormat
    );
}

#[test]
fn test_extract_from_authorization_header_whitespace_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer    "));

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::InvalidAuthorizationFormat
    );
}

#[test]
fn test_extract_from_authorization_header_case_sensitive_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("bearer test_token"),
    );

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        TokenExtractionError::InvalidAuthorizationFormat
    );
}

#[test]
fn test_extract_from_authorization_multiple_headers_first_valid() {
    let mut headers = HeaderMap::new();
    headers.append(
        "authorization",
        HeaderValue::from_static("Bearer first_token"),
    );
    headers.append(
        "authorization",
        HeaderValue::from_static("Bearer second_token"),
    );

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "first_token");
}

#[test]
fn test_extract_from_authorization_multiple_headers_skip_invalid() {
    let mut headers = HeaderMap::new();
    headers.append("authorization", HeaderValue::from_static("Basic invalid"));
    headers.append(
        "authorization",
        HeaderValue::from_static("Bearer valid_token"),
    );

    let result = TokenExtractor::extract_from_authorization(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "valid_token");
}

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
    // Cookie parsing extracts value up to the next semicolon (trimmed)
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
