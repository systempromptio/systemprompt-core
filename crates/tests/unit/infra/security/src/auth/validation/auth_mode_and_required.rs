//! Tests for AuthMode enum variants and AuthMode::Required/Disabled validation

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_models::auth::{JwtAudience, UserType};
use systemprompt_security::{AuthMode, AuthValidationService};

use super::helpers::*;

// ============================================================================
// AuthMode Tests
// ============================================================================

#[test]
fn test_auth_mode_required_variant() {
    let mode = AuthMode::Required;
    assert_eq!(mode, AuthMode::Required);
}

#[test]
fn test_auth_mode_optional_variant() {
    let mode = AuthMode::Optional;
    assert_eq!(mode, AuthMode::Optional);
}

#[test]
fn test_auth_mode_disabled_variant() {
    let mode = AuthMode::Disabled;
    assert_eq!(mode, AuthMode::Disabled);
}

#[test]
fn test_auth_mode_clone() {
    let mode = AuthMode::Required;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

// ============================================================================
// AuthValidationService Constructor Tests
// ============================================================================

#[test]
fn test_auth_validation_service_new() {
    let service = AuthValidationService::new(
        "secret".to_string(),
        "issuer".to_string(),
        vec![JwtAudience::Api],
    );
    let debug_str = format!("{:?}", service);
    assert!(debug_str.contains("AuthValidationService"));
}

#[test]
fn test_auth_validation_service_debug() {
    let service = create_test_service();
    let debug_str = format!("{:?}", service);
    assert!(debug_str.contains("AuthValidationService"));
}

// ============================================================================
// AuthMode::Disabled Tests
// ============================================================================

#[test]
fn test_validate_request_disabled_mode() {
    let service = create_test_service();
    let headers = HeaderMap::new();

    let context = service
        .validate_request(&headers, AuthMode::Disabled)
        .expect("Disabled mode should succeed without token");
    assert_eq!(context.request.session_id.as_str(), "test");
    assert_eq!(context.auth.user_id.as_str(), "test-user");
}

#[test]
fn test_validate_request_disabled_ignores_token() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer invalid_token"),
    );

    service
        .validate_request(&headers, AuthMode::Disabled)
        .expect("Disabled mode should ignore invalid token");
}

// ============================================================================
// AuthMode::Required Tests
// ============================================================================

#[test]
fn test_validate_request_required_missing_auth() {
    let service = create_test_service();
    let headers = HeaderMap::new();

    let err = service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
    assert!(err.to_string().contains("Missing authorization"));
}

#[test]
fn test_validate_request_required_invalid_token() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer invalid_token_format"),
    );

    let err = service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
    assert!(err.to_string().contains("Invalid JWT"));
}

#[test]
fn test_validate_request_required_wrong_secret() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "wrong_secret",
        "test_issuer",
        Some("session_123".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
}

#[test]
fn test_validate_request_required_wrong_issuer() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "wrong_issuer",
        Some("session_123".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
}

#[test]
fn test_validate_request_required_expired_token() {
    let service = create_test_service();
    let token = create_expired_jwt("test_secret_key", "test_issuer");

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let err = service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
    assert!(err.to_string().contains("Invalid JWT"));
}

#[test]
fn test_validate_request_required_missing_session_id() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", None);

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let err = service
        .validate_request(&headers, AuthMode::Required)
        .unwrap_err();
    assert!(err.to_string().contains("session_id"));
}

#[test]
fn test_validate_request_required_valid_token() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session_123".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let context = service
        .validate_request(&headers, AuthMode::Required)
        .expect("Valid token should succeed");
    assert_eq!(context.auth.user_id.as_str(), "user_123");
    assert_eq!(context.request.session_id.as_str(), "session_123");
}

#[test]
fn test_validate_request_required_admin_token() {
    let service = create_test_service();
    let token = create_admin_jwt("test_secret_key", "test_issuer");

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let context = service
        .validate_request(&headers, AuthMode::Required)
        .expect("Admin token should succeed");
    assert_eq!(context.auth.user_type, UserType::Admin);
}
