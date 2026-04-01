//! Tests for redirect URI validation

use systemprompt_oauth::services::validation::validate_redirect_uri;
use systemprompt_models::AuthError;

// ============================================================================
// validate_redirect_uri Tests
// ============================================================================

#[test]
fn test_validate_redirect_uri_success() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback"));

    assert_eq!(result.expect("valid redirect URI should succeed"), "https://example.com/callback");
}

#[test]
fn test_validate_redirect_uri_multiple_registered() {
    let registered = vec![
        "https://example.com/callback1".to_string(),
        "https://example.com/callback2".to_string(),
        "https://example.com/callback3".to_string(),
    ];

    let result = validate_redirect_uri(&registered, Some("https://example.com/callback2"));
    assert_eq!(result.expect("valid redirect URI should succeed"), "https://example.com/callback2");
}

#[test]
fn test_validate_redirect_uri_none() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, None);

    assert!(matches!(result.unwrap_err(), AuthError::InvalidRedirectUri));
}

#[test]
fn test_validate_redirect_uri_empty_string() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some(""));

    assert!(matches!(result.unwrap_err(), AuthError::InvalidRedirectUri));
}

#[test]
fn test_validate_redirect_uri_not_registered() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://evil.com/callback"));

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
    result.expect("same case should match");

    // Different case should not match
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback"));
    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_with_query_params() {
    let registered = vec!["https://example.com/callback?foo=bar".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback?foo=bar"));

    result.expect("URI with query params should match");
}

#[test]
fn test_validate_redirect_uri_partial_match_fails() {
    let registered = vec!["https://example.com/callback".to_string()];

    // Should not match partial URIs
    let result = validate_redirect_uri(&registered, Some("https://example.com/callbac"));
    result.unwrap_err();

    let result = validate_redirect_uri(&registered, Some("https://example.com/callback/extra"));
    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_localhost() {
    let registered = vec!["http://localhost:8080/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("http://localhost:8080/callback"));

    result.expect("localhost URI should match");
}

#[test]
fn test_validate_redirect_uri_localhost_127() {
    let registered = vec!["http://127.0.0.1:3000/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("http://127.0.0.1:3000/callback"));

    result.expect("127.0.0.1 URI should match");
}

#[test]
fn test_validate_redirect_uri_custom_scheme() {
    let registered = vec!["myapp://callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("myapp://callback"));

    result.expect("custom scheme URI should match");
}

#[test]
fn test_validate_redirect_uri_with_port() {
    let registered = vec!["https://example.com:8443/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com:8443/callback"));

    result.expect("URI with port should match");
}

#[test]
fn test_validate_redirect_uri_port_mismatch() {
    let registered = vec!["https://example.com:8443/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com:9443/callback"));

    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_with_fragment() {
    let registered = vec!["https://example.com/callback#section".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/callback#section"));

    result.expect("URI with fragment should match");
}

#[test]
fn test_validate_redirect_uri_whitespace_only() {
    let registered = vec!["https://example.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("   "));

    result.unwrap_err();
}

// ============================================================================
// Relative URI matching tests
// ============================================================================

#[test]
fn test_validate_redirect_uri_relative_path_matches_absolute() {
    let registered = vec!["/admin/login".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/admin/login"));

    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("not registered"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_validate_redirect_uri_relative_path_different_host() {
    let registered = vec!["/admin/login".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://other-host.io/admin/login"));

    match result.unwrap_err() {
        AuthError::InvalidRequest { reason } => {
            assert!(reason.contains("not registered"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_validate_redirect_uri_relative_path_no_match() {
    let registered = vec!["/admin/login".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/other/path"));

    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_relative_path_partial_no_match() {
    let registered = vec!["/admin/login".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com/admin/login/extra"));

    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_protocol_relative_not_treated_as_path() {
    // URIs starting with // should NOT be treated as relative paths
    let registered = vec!["//evil.com/callback".to_string()];
    let result = validate_redirect_uri(&registered, Some("https://example.com//evil.com/callback"));

    result.unwrap_err();
}

#[test]
fn test_validate_redirect_uri_relative_alongside_absolute() {
    let registered = vec![
        "/callback".to_string(),
        "http://localhost:8080/callback".to_string(),
    ];

    // Absolute match still works
    let result = validate_redirect_uri(&registered, Some("http://localhost:8080/callback"));
    result.expect("absolute match should work");

    // Absolute URI no longer matches relative registered path
    let result = validate_redirect_uri(&registered, Some("https://prod.example.com/callback"));
    result.unwrap_err();
}
