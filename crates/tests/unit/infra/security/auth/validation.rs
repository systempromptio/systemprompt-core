//! Unit tests for AuthValidationService, AuthMode, and TokenClaims
//!
//! Tests cover:
//! - AuthMode enum variants
//! - AuthValidationService request validation
//! - Token extraction and validation
//! - Context creation from claims

use axum::http::{HeaderMap, HeaderValue};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use systemprompt_core_security::{AuthMode, AuthValidationService};
use systemprompt_models::auth::{JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType};

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_service() -> AuthValidationService {
    AuthValidationService::new(
        "test_secret_key".to_string(),
        "test_issuer".to_string(),
        JwtAudience::standard(),
    )
}

fn create_valid_jwt(secret: &str, issuer: &str, session_id: Option<String>) -> String {
    let now = Utc::now();
    let expiry = now + Duration::hours(1);

    let claims = JwtClaims {
        sub: "user_123".to_string(),
        iat: now.timestamp(),
        exp: expiry.timestamp(),
        iss: issuer.to_string(),
        aud: JwtAudience::standard(),
        jti: "jti_123".to_string(),
        scope: vec![Permission::User],
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        user_type: UserType::Standard,
        client_id: Some("test_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id,
        rate_limit_tier: Some(RateLimitTier::Standard),
    };

    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
}

fn create_admin_jwt(secret: &str, issuer: &str) -> String {
    let now = Utc::now();
    let expiry = now + Duration::hours(1);

    let claims = JwtClaims {
        sub: "admin_user".to_string(),
        iat: now.timestamp(),
        exp: expiry.timestamp(),
        iss: issuer.to_string(),
        aud: JwtAudience::standard(),
        jti: "jti_admin".to_string(),
        scope: vec![Permission::Admin, Permission::User],
        username: "admin".to_string(),
        email: "admin@example.com".to_string(),
        user_type: UserType::Standard,
        client_id: Some("admin_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some("admin_session".to_string()),
        rate_limit_tier: Some(RateLimitTier::Admin),
    };

    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
}

fn create_expired_jwt(secret: &str, issuer: &str) -> String {
    let now = Utc::now();
    let expiry = now - Duration::hours(1);

    let claims = JwtClaims {
        sub: "user_123".to_string(),
        iat: (now - Duration::hours(2)).timestamp(),
        exp: expiry.timestamp(),
        iss: issuer.to_string(),
        aud: JwtAudience::standard(),
        jti: "jti_expired".to_string(),
        scope: vec![Permission::User],
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        user_type: UserType::Standard,
        client_id: Some("test_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: (now - Duration::hours(2)).timestamp(),
        session_id: Some("session_123".to_string()),
        rate_limit_tier: Some(RateLimitTier::Standard),
    };

    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
}

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
fn test_auth_mode_debug() {
    assert!(format!("{:?}", AuthMode::Required).contains("Required"));
    assert!(format!("{:?}", AuthMode::Optional).contains("Optional"));
    assert!(format!("{:?}", AuthMode::Disabled).contains("Disabled"));
}

#[test]
fn test_auth_mode_clone() {
    let mode = AuthMode::Required;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_auth_mode_copy() {
    let mode = AuthMode::Optional;
    let copied: AuthMode = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_auth_mode_equality() {
    assert_eq!(AuthMode::Required, AuthMode::Required);
    assert_eq!(AuthMode::Optional, AuthMode::Optional);
    assert_eq!(AuthMode::Disabled, AuthMode::Disabled);
    assert_ne!(AuthMode::Required, AuthMode::Optional);
    assert_ne!(AuthMode::Required, AuthMode::Disabled);
    assert_ne!(AuthMode::Optional, AuthMode::Disabled);
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

    let result = service.validate_request(&headers, AuthMode::Disabled);
    assert!(result.is_ok());

    let context = result.unwrap();
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

    let result = service.validate_request(&headers, AuthMode::Disabled);
    assert!(result.is_ok());
}

// ============================================================================
// AuthMode::Required Tests
// ============================================================================

#[test]
fn test_validate_request_required_missing_auth() {
    let service = create_test_service();
    let headers = HeaderMap::new();

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Missing authorization"));
}

#[test]
fn test_validate_request_required_invalid_token() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer invalid_token_format"),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid JWT"));
}

#[test]
fn test_validate_request_required_wrong_secret() {
    let service = create_test_service();
    let token = create_valid_jwt("wrong_secret", "test_issuer", Some("session_123".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_required_wrong_issuer() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "wrong_issuer", Some("session_123".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
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

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid JWT"));
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

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("session_id"));
}

#[test]
fn test_validate_request_required_valid_token() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session_123".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
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

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.auth.user_type, UserType::Admin);
}

// ============================================================================
// AuthMode::Optional Tests
// ============================================================================

#[test]
fn test_validate_request_optional_no_token() {
    let service = create_test_service();
    let headers = HeaderMap::new();

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.request.session_id.as_str(), "anonymous");
    assert_eq!(context.auth.user_type, UserType::Anon);
}

#[test]
fn test_validate_request_optional_invalid_token() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer invalid_token"),
    );

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.request.session_id.as_str(), "anonymous");
    assert_eq!(context.auth.user_type, UserType::Anon);
}

#[test]
fn test_validate_request_optional_valid_token() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session_456".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.auth.user_id.as_str(), "user_123");
    assert_eq!(context.request.session_id.as_str(), "session_456");
    assert_eq!(context.auth.user_type, UserType::Standard);
}

// ============================================================================
// Header Extraction Tests
// ============================================================================

#[test]
fn test_validate_request_extracts_trace_id() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-trace-id", HeaderValue::from_static("custom-trace-id"));

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.trace_id.as_str(), "custom-trace-id");
}

#[test]
fn test_validate_request_extracts_context_id() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-context-id", HeaderValue::from_static("custom-context-id"));

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.context_id.as_str(), "custom-context-id");
}

#[test]
fn test_validate_request_extracts_agent_name() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-agent-name", HeaderValue::from_static("custom-agent"));

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.agent_name.as_str(), "custom-agent");
}

#[test]
fn test_validate_request_generates_trace_id_if_missing() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert!(context.execution.trace_id.as_str().starts_with("trace_"));
}

#[test]
fn test_validate_request_anonymous_extracts_headers() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("anon-trace"));
    headers.insert("x-context-id", HeaderValue::from_static("anon-context"));
    headers.insert("x-agent-name", HeaderValue::from_static("anon-agent"));

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.trace_id.as_str(), "anon-trace");
    assert_eq!(context.execution.context_id.as_str(), "anon-context");
    assert_eq!(context.execution.agent_name.as_str(), "anon-agent");
}

// ============================================================================
// Authorization Header Format Tests
// ============================================================================

#[test]
fn test_validate_request_lowercase_authorization() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());
}

#[test]
fn test_validate_request_no_bearer_prefix() {
    let service = create_test_service();
    let token = create_valid_jwt("test_secret_key", "test_issuer", Some("session".to_string()));

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&token).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_basic_auth() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
}
