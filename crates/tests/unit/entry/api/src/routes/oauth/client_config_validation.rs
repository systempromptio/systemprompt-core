use http::HeaderMap;
use http::header::HeaderValue;
use systemprompt_api::routes::oauth::endpoints::client_config::validation::validate_registration_token;

// ============================================================================
// Missing Header Tests
// ============================================================================

#[test]
fn test_missing_authorization_header_returns_error() {
    let headers = HeaderMap::new();
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

// ============================================================================
// Invalid Header Format Tests
// ============================================================================

#[test]
fn test_non_utf8_header_value_returns_error() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_bytes(&[0x80, 0x81]).unwrap(),
    );
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

// ============================================================================
// Bearer Scheme Validation Tests
// ============================================================================

#[test]
fn test_basic_auth_instead_of_bearer_returns_error() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Basic dXNlcjpwYXNz".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_lowercase_bearer_returns_error() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer reg_abc123".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

// ============================================================================
// Registration Token Prefix Validation Tests
// ============================================================================

#[test]
fn test_bearer_without_reg_prefix_returns_error() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer sometoken".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

#[test]
fn test_empty_bearer_value_returns_error() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_err());
}

// ============================================================================
// Valid Token Tests
// ============================================================================

#[test]
fn test_valid_bearer_reg_token_returns_ok() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer reg_abc123".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "reg_abc123");
}

#[test]
fn test_bearer_with_just_reg_prefix_returns_ok() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer reg_".parse().unwrap());
    let result = validate_registration_token(&headers);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "reg_");
}
