//! Unit tests for CookieExtractor
//!
//! Tests cover:
//! - Cookie extraction with default cookie name
//! - Cookie extraction with custom cookie name
//! - Static extraction method
//! - Error handling for missing/invalid cookies

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_security::{CookieExtractionError, CookieExtractor};

// ============================================================================
// CookieExtractor Constructor Tests
// ============================================================================

#[test]
fn test_cookie_extractor_new() {
    let extractor = CookieExtractor::new("my_token");
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("my_token=test_value"));

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test_value");
}

#[test]
fn test_cookie_extractor_default() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=default_value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "default_value");
}

#[test]
fn test_cookie_extractor_default_cookie_name_constant() {
    assert_eq!(CookieExtractor::DEFAULT_COOKIE_NAME, "access_token");
}

// ============================================================================
// Static Extraction Tests
// ============================================================================

#[test]
fn test_extract_access_token_static() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=static_token"),
    );

    let result = CookieExtractor::extract_access_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "static_token");
}

#[test]
fn test_extract_access_token_static_missing() {
    let headers = HeaderMap::new();

    let result = CookieExtractor::extract_access_token(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), CookieExtractionError::MissingCookie);
}

// ============================================================================
// Cookie Extraction Success Cases
// ============================================================================

#[test]
fn test_extract_single_cookie() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=single_value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "single_value");
}

#[test]
fn test_extract_cookie_from_multiple() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("session=sess123; access_token=token456; theme=dark"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "token456");
}

#[test]
fn test_extract_cookie_first_in_list() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=first_token; other=value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "first_token");
}

#[test]
fn test_extract_cookie_last_in_list() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("other=value; access_token=last_token"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "last_token");
}

#[test]
fn test_extract_cookie_with_special_characters() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static(
            "access_token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.\
             dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
        ),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert!(result.unwrap().starts_with("eyJhbGciOiJIUzI1NiI"));
}

#[test]
fn test_extract_cookie_with_spaces_around_semicolons() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("  session=abc  ;  access_token=spaced  ;  other=val  "),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    // Cookie value extraction trims the cookie key prefix but not trailing spaces
    assert_eq!(result.unwrap(), "spaced");
}

// ============================================================================
// Cookie Extraction Error Cases
// ============================================================================

#[test]
fn test_extract_missing_cookie_header() {
    let extractor = CookieExtractor::default();
    let headers = HeaderMap::new();

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), CookieExtractionError::MissingCookie);
}

#[test]
fn test_extract_token_not_in_cookie() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("session=abc; theme=dark"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        CookieExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_empty_cookie_value() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static("access_token="));

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        CookieExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_empty_cookie_header() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", HeaderValue::from_static(""));

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        CookieExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_partial_cookie_name_match() {
    let extractor = CookieExtractor::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token_backup=wrong; my_access_token=also_wrong"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        CookieExtractionError::TokenNotFoundInCookie
    );
}

// ============================================================================
// Custom Cookie Name Tests
// ============================================================================

#[test]
fn test_extract_custom_cookie_name() {
    let extractor = CookieExtractor::new("auth_token");
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("auth_token=custom_value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "custom_value");
}

#[test]
fn test_extract_custom_cookie_not_default() {
    let extractor = CookieExtractor::new("auth_token");
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("access_token=wrong_cookie"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        CookieExtractionError::TokenNotFoundInCookie
    );
}

#[test]
fn test_extract_custom_cookie_from_string() {
    let extractor = CookieExtractor::new(String::from("dynamic_name"));
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        HeaderValue::from_static("dynamic_name=dynamic_value"),
    );

    let result = extractor.extract(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "dynamic_value");
}

// ============================================================================
// CookieExtractionError Display Tests
// ============================================================================

#[test]
fn test_cookie_extraction_error_display_missing() {
    let error = CookieExtractionError::MissingCookie;
    assert_eq!(format!("{}", error), "Missing cookie header");
}

#[test]
fn test_cookie_extraction_error_display_invalid_format() {
    let error = CookieExtractionError::InvalidCookieFormat;
    assert_eq!(format!("{}", error), "Invalid cookie format");
}

#[test]
fn test_cookie_extraction_error_display_not_found() {
    let error = CookieExtractionError::TokenNotFoundInCookie;
    assert_eq!(format!("{}", error), "Access token not found in cookies");
}

#[test]
fn test_cookie_extraction_error_is_std_error() {
    let error: Box<dyn std::error::Error> = Box::new(CookieExtractionError::MissingCookie);
    assert!(error.to_string().contains("Missing cookie"));
}

#[test]
fn test_cookie_extraction_error_equality() {
    assert_eq!(
        CookieExtractionError::MissingCookie,
        CookieExtractionError::MissingCookie
    );
    assert_eq!(
        CookieExtractionError::InvalidCookieFormat,
        CookieExtractionError::InvalidCookieFormat
    );
    assert_eq!(
        CookieExtractionError::TokenNotFoundInCookie,
        CookieExtractionError::TokenNotFoundInCookie
    );
    assert_ne!(
        CookieExtractionError::MissingCookie,
        CookieExtractionError::InvalidCookieFormat
    );
}

#[test]
fn test_cookie_extraction_error_clone() {
    let error = CookieExtractionError::MissingCookie;
    let cloned = error;
    assert_eq!(error, cloned);
}

#[test]
fn test_cookie_extraction_error_debug() {
    let error = CookieExtractionError::MissingCookie;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("MissingCookie"));
}
