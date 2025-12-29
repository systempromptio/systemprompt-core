//! Tests for redirect URI validation

use systemprompt_core_oauth::services::validation::validate_redirect_uri;
use systemprompt_models::AuthError;

// ============================================================================
// validate_redirect_uri Tests
// ============================================================================

#[test]
fn test_validate_redirect_uri_success() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback"));

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://example.com/callback");
}

#[test]
fn test_validate_redirect_uri_multiple_registered() {
    let registered = vec![
        "https://example.com/callback1".to_string(),
        "https://example.com/callback2".to_string(),
        "https://example.com/callback3".to_string(),
    ];

    let result = validate_redirect_uri(&registered, Some("https://example.com/callback2"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://example.com/callback2");
}

#[test]
fn test_validate_redirect_uri_none() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, None);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::InvalidRedirectUri));
}

#[test]
fn test_validate_redirect_uri_empty_string() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some(""));

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AuthError::InvalidRedirectUri));
}

#[test]
fn test_validate_redirect_uri_not_registered() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://evil.com/callback"));

    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("not registered"));
            assert!(reason.contains("https://evil.com/callback"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_validate_redirect_uri_empty_registered_list() {
    let registered: Vec<String> = vec![];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback"));

    assert!(result.is_err());
    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("not registered"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_validate_redirect_uri_case_sensitive() {
    let registered = vec!["https://Example.com/callback".to_string()];

    // Same case should match
    let result = validate_redirect_uri(&registered, Some("https://Example.com/callback"));
    assert!(result.is_ok());

    // Different case should not match
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback"));
    assert!(result.is_err());
}

#[test]
fn test_validate_redirect_uri_with_query_params() {
    let registered = vec!["https://example.com/callback?foo=bar".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback?foo=bar"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_partial_match_fails() {
    let registered = vec!["https://example.com/callback".to_string()];

    // Should not match partial URIs
    let result = validate_redirect_uri(&registered, Some("https://example.com/callbac"));
    assert!(result.is_err());

    let result = validate_redirect_uri(&registered, Some("https://example.com/callback/extra"));
    assert!(result.is_err());
}

#[test]
fn test_validate_redirect_uri_localhost() {
    let registered = vec!["http://localhost:8080/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("http://localhost:8080/callback"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_localhost_127() {
    let registered = vec!["http://127.0.0.1:3000/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("http://127.0.0.1:3000/callback"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_custom_scheme() {
    let registered = vec!["myapp://callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("myapp://callback"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_with_port() {
    let registered = vec!["https://example.com:8443/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com:8443/callback"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_port_mismatch() {
    let registered = vec!["https://example.com:8443/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com:9443/callback"));

    assert!(result.is_err());
}

#[test]
fn test_validate_redirect_uri_with_fragment() {
    let registered = vec!["https://example.com/callback#section".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback#section"));

    assert!(result.is_ok());
}

#[test]
fn test_validate_redirect_uri_whitespace_only() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("   "));

    assert!(result.is_err());
}
