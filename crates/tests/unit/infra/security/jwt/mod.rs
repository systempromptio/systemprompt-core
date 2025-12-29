//! Unit tests for JwtService
//!
//! Tests cover:
//! - Admin token generation
//! - Token structure and claims
//! - AdminTokenParams configuration

use chrono::Duration;
use systemprompt_core_security::{AdminTokenParams, JwtService};
use systemprompt_identifiers::{SessionId, UserId};

// ============================================================================
// AdminTokenParams Tests
// ============================================================================

#[test]
fn test_admin_token_params_creation() {
    let user_id = UserId::new("user_123".to_string());
    let session_id = SessionId::new("session_456".to_string());

    let params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "admin@example.com",
        jwt_secret: "test_secret_key_for_testing",
        issuer: "test_issuer",
        duration: Duration::hours(1),
    };

    assert_eq!(params.email, "admin@example.com");
    assert_eq!(params.issuer, "test_issuer");
}

#[test]
fn test_admin_token_params_debug() {
    let user_id = UserId::new("user_123".to_string());
    let session_id = SessionId::new("session_456".to_string());

    let params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "admin@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let debug_str = format!("{:?}", params);
    assert!(debug_str.contains("AdminTokenParams"));
}

// ============================================================================
// JwtService Token Generation Tests
// ============================================================================

#[test]
fn test_generate_admin_token_success() {
    let user_id = UserId::new("admin_user_id".to_string());
    let session_id = SessionId::new("admin_session_id".to_string());

    let params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "admin@systemprompt.io",
        jwt_secret: "a_very_secure_secret_key_for_jwt_signing",
        issuer: "systemprompt",
        duration: Duration::hours(24),
    };

    let result = JwtService::generate_admin_token(&params);
    assert!(result.is_ok());

    let token = result.unwrap();
    let token_str = token.as_str();

    assert!(!token_str.is_empty());
    assert_eq!(token_str.split('.').count(), 3);
}

#[test]
fn test_generate_admin_token_structure() {
    let user_id = UserId::new("user_id".to_string());
    let session_id = SessionId::new("session_id".to_string());

    let params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret_key",
        issuer: "test_issuer",
        duration: Duration::minutes(30),
    };

    let token = JwtService::generate_admin_token(&params).unwrap();
    let parts: Vec<&str> = token.as_str().split('.').collect();

    assert_eq!(parts.len(), 3);
    assert!(!parts[0].is_empty());
    assert!(!parts[1].is_empty());
    assert!(!parts[2].is_empty());
}

#[test]
fn test_generate_admin_token_different_durations() {
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let short_params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::minutes(5),
    };

    let long_params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::days(30),
    };

    let short_token = JwtService::generate_admin_token(&short_params);
    let long_token = JwtService::generate_admin_token(&long_params);

    assert!(short_token.is_ok());
    assert!(long_token.is_ok());

    assert_ne!(short_token.unwrap().as_str(), long_token.unwrap().as_str());
}

#[test]
fn test_generate_admin_token_different_secrets() {
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params1 = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret_one",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let params2 = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret_two",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let token1 = JwtService::generate_admin_token(&params1).unwrap();
    let token2 = JwtService::generate_admin_token(&params2).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_admin_token_different_users() {
    let user_id1 = UserId::new("user_one".to_string());
    let user_id2 = UserId::new("user_two".to_string());
    let session_id = SessionId::new("session".to_string());

    let params1 = AdminTokenParams {
        user_id: &user_id1,
        session_id: &session_id,
        email: "user1@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let params2 = AdminTokenParams {
        user_id: &user_id2,
        session_id: &session_id,
        email: "user2@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let token1 = JwtService::generate_admin_token(&params1).unwrap();
    let token2 = JwtService::generate_admin_token(&params2).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_admin_token_different_sessions() {
    let user_id = UserId::new("user".to_string());
    let session_id1 = SessionId::new("session_one".to_string());
    let session_id2 = SessionId::new("session_two".to_string());

    let params1 = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id1,
        email: "test@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let params2 = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id2,
        email: "test@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let token1 = JwtService::generate_admin_token(&params1).unwrap();
    let token2 = JwtService::generate_admin_token(&params2).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_admin_token_unique_jti() {
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        jwt_secret: "secret",
        issuer: "issuer",
        duration: Duration::hours(1),
    };

    let token1 = JwtService::generate_admin_token(&params).unwrap();
    let token2 = JwtService::generate_admin_token(&params).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

// ============================================================================
// JwtService Struct Tests
// ============================================================================

#[test]
fn test_jwt_service_debug() {
    let service = JwtService;
    let debug_str = format!("{:?}", service);
    assert!(debug_str.contains("JwtService"));
}

#[test]
fn test_jwt_service_clone() {
    let service = JwtService;
    let cloned = service;
    let _ = format!("{:?}", cloned);
}

#[test]
fn test_jwt_service_copy() {
    let service = JwtService;
    let copied: JwtService = service;
    let _ = format!("{:?}", copied);
}
