//! Tests for ExtractionMethod display, TokenExtractor constructors, and
//! authorization header extraction

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_security::{ExtractionMethod, TokenExtractionError, TokenExtractor};

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

    let token = extractor
        .extract(&headers)
        .expect("Should extract from custom cookie");
    assert_eq!(token, "my_token_value");
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

    let token = extractor
        .extract(&headers)
        .expect("Should extract from custom MCP header");
    assert_eq!(token, "custom_token");
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

    let token = TokenExtractor::extract_from_authorization(&headers)
        .expect("Should extract from authorization header");
    assert_eq!(token, "test_token_123");
}

#[test]
fn test_extract_from_authorization_header_missing() {
    let headers = HeaderMap::new();

    let err = TokenExtractor::extract_from_authorization(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::MissingAuthorizationHeader);
}

#[test]
fn test_extract_from_authorization_header_invalid_format_no_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );

    let err = TokenExtractor::extract_from_authorization(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::InvalidAuthorizationFormat);
}

#[test]
fn test_extract_from_authorization_header_empty_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer "));

    let err = TokenExtractor::extract_from_authorization(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::InvalidAuthorizationFormat);
}

#[test]
fn test_extract_from_authorization_header_whitespace_token() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer    "));

    let err = TokenExtractor::extract_from_authorization(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::InvalidAuthorizationFormat);
}

#[test]
fn test_extract_from_authorization_header_case_sensitive_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("bearer test_token"),
    );

    let err = TokenExtractor::extract_from_authorization(&headers).unwrap_err();
    assert_eq!(err, TokenExtractionError::InvalidAuthorizationFormat);
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

    let token = TokenExtractor::extract_from_authorization(&headers)
        .expect("Should extract first valid token");
    assert_eq!(token, "first_token");
}

#[test]
fn test_extract_from_authorization_multiple_headers_skip_invalid() {
    let mut headers = HeaderMap::new();
    headers.append("authorization", HeaderValue::from_static("Basic invalid"));
    headers.append(
        "authorization",
        HeaderValue::from_static("Bearer valid_token"),
    );

    let token = TokenExtractor::extract_from_authorization(&headers)
        .expect("Should skip invalid and extract valid token");
    assert_eq!(token, "valid_token");
}
