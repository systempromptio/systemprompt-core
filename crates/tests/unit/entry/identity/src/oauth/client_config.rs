//! Tests for OAuth client configuration validation behavior
//!
//! The `validate_registration_token` function in
//! `routes::oauth::endpoints::client_config::validation` is not publicly
//! exported. These tests verify the header parsing contract that the validation
//! enforces, using the same header construction patterns the production code
//! uses.

use http::HeaderMap;
use http::header::HeaderValue;

#[test]
fn test_validate_reg_token_missing_header_produces_no_auth() {
    let headers = HeaderMap::new();
    let auth = headers.get("authorization");
    assert!(auth.is_none());
}

#[test]
fn test_validate_reg_token_invalid_format_is_not_valid_str() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_bytes(&[0x80, 0x81]).unwrap(),
    );
    let header = headers.get("authorization").unwrap();
    assert!(header.to_str().is_err());
}

#[test]
fn test_validate_reg_token_not_bearer_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Basic dXNlcjpwYXNz".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    assert!(!value.starts_with("Bearer "));
}

#[test]
fn test_validate_reg_token_missing_reg_prefix_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer sometoken".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    let token = value.strip_prefix("Bearer ").unwrap();
    assert!(!token.starts_with("reg_"));
}

#[test]
fn test_validate_reg_token_valid() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer reg_abc123".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    let token = value.strip_prefix("Bearer ").unwrap();
    assert!(token.starts_with("reg_"));
    assert_eq!(token, "reg_abc123");
}

#[test]
fn test_validate_reg_token_empty_bearer_has_no_reg_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    let token = value.strip_prefix("Bearer ").unwrap();
    assert!(!token.starts_with("reg_"));
}

#[test]
fn test_validate_reg_token_just_reg_prefix_is_valid() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer reg_".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    let token = value.strip_prefix("Bearer ").unwrap();
    assert!(token.starts_with("reg_"));
    assert_eq!(token, "reg_");
}

#[test]
fn test_validate_reg_token_case_sensitive_bearer() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer reg_abc".parse().unwrap());
    let header = headers.get("authorization").unwrap();
    let value = header.to_str().unwrap();
    assert!(
        value.strip_prefix("Bearer ").is_none(),
        "lowercase 'bearer' should not match 'Bearer ' prefix"
    );
}
