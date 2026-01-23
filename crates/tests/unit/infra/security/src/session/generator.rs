//! Unit tests for SessionGenerator and SessionParams
//!
//! Tests cover:
//! - SessionGenerator construction
//! - Session token generation
//! - SessionParams struct
//! - Token structure validation
//! - Different parameter combinations

use chrono::Duration;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::{SessionGenerator, SessionParams};

fn create_test_generator() -> SessionGenerator {
    SessionGenerator::new("test_jwt_secret_key", "test_issuer")
}

fn create_test_params<'a>(user_id: &'a UserId, session_id: &'a SessionId) -> SessionParams<'a> {
    SessionParams {
        user_id,
        session_id,
        email: "test@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    }
}

#[test]
fn test_session_generator_new() {
    let generator = SessionGenerator::new("secret", "issuer");
    let debug_str = format!("{:?}", generator);
    assert!(debug_str.contains("SessionGenerator"));
}

#[test]
fn test_session_generator_new_with_string() {
    let generator = SessionGenerator::new(String::from("secret"), String::from("issuer"));
    let debug_str = format!("{:?}", generator);
    assert!(debug_str.contains("SessionGenerator"));
}

#[test]
fn test_session_generator_debug() {
    let generator = create_test_generator();
    let debug_str = format!("{:?}", generator);
    assert!(debug_str.contains("SessionGenerator"));
}

#[test]
fn test_session_params_debug() {
    let user_id = UserId::new("user_123".to_string());
    let session_id = SessionId::new("session_456".to_string());
    let params = create_test_params(&user_id, &session_id);

    let debug_str = format!("{:?}", params);
    assert!(debug_str.contains("SessionParams"));
}

#[test]
fn test_generate_session_token_success() {
    let generator = create_test_generator();
    let user_id = UserId::new("user_123".to_string());
    let session_id = SessionId::new("session_456".to_string());
    let params = create_test_params(&user_id, &session_id);

    let result = generator.generate(&params);
    assert!(result.is_ok());

    let token = result.unwrap();
    assert!(!token.as_str().is_empty());
}

#[test]
fn test_generate_session_token_jwt_structure() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());
    let params = create_test_params(&user_id, &session_id);

    let token = generator.generate(&params).unwrap();
    let parts: Vec<&str> = token.as_str().split('.').collect();

    assert_eq!(parts.len(), 3, "JWT should have 3 parts (header.payload.signature)");
    assert!(!parts[0].is_empty(), "Header should not be empty");
    assert!(!parts[1].is_empty(), "Payload should not be empty");
    assert!(!parts[2].is_empty(), "Signature should not be empty");
}

#[test]
fn test_generate_session_token_unique_tokens() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());
    let params = create_test_params(&user_id, &session_id);

    let token1 = generator.generate(&params).unwrap();
    let token2 = generator.generate(&params).unwrap();

    assert_ne!(
        token1.as_str(),
        token2.as_str(),
        "Each token should have unique jti"
    );
}

#[test]
fn test_generate_session_token_different_users() {
    let generator = create_test_generator();
    let user_id1 = UserId::new("user_one".to_string());
    let user_id2 = UserId::new("user_two".to_string());
    let session_id = SessionId::new("session".to_string());

    let params1 = SessionParams {
        user_id: &user_id1,
        session_id: &session_id,
        email: "user1@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let params2 = SessionParams {
        user_id: &user_id2,
        session_id: &session_id,
        email: "user2@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let token1 = generator.generate(&params1).unwrap();
    let token2 = generator.generate(&params2).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_session_token_different_sessions() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id1 = SessionId::new("session_one".to_string());
    let session_id2 = SessionId::new("session_two".to_string());

    let params1 = create_test_params(&user_id, &session_id1);
    let params2 = create_test_params(&user_id, &session_id2);

    let token1 = generator.generate(&params1).unwrap();
    let token2 = generator.generate(&params2).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_session_token_different_secrets() {
    let generator1 = SessionGenerator::new("secret_one", "issuer");
    let generator2 = SessionGenerator::new("secret_two", "issuer");
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());
    let params = create_test_params(&user_id, &session_id);

    let token1 = generator1.generate(&params).unwrap();
    let token2 = generator2.generate(&params).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_session_token_different_issuers() {
    let generator1 = SessionGenerator::new("secret", "issuer_one");
    let generator2 = SessionGenerator::new("secret", "issuer_two");
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());
    let params = create_test_params(&user_id, &session_id);

    let token1 = generator1.generate(&params).unwrap();
    let token2 = generator2.generate(&params).unwrap();

    assert_ne!(token1.as_str(), token2.as_str());
}

#[test]
fn test_generate_session_token_admin_user_type() {
    let generator = create_test_generator();
    let user_id = UserId::new("admin".to_string());
    let session_id = SessionId::new("admin_session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "admin@example.com",
        duration: Duration::hours(8),
        user_type: UserType::Admin,
        permissions: vec![Permission::Admin, Permission::User],
        roles: vec!["admin".to_string(), "user".to_string()],
        rate_limit_tier: RateLimitTier::Admin,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_short_duration() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        duration: Duration::minutes(5),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_long_duration() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        duration: Duration::days(30),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_multiple_permissions() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User, Permission::Admin],
        roles: vec!["user".to_string(), "admin".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_empty_roles() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec![],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_empty_permissions() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "test@example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_session_params_all_rate_limit_tiers() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let tiers = [
        RateLimitTier::User,
        RateLimitTier::Admin,
        RateLimitTier::A2a,
        RateLimitTier::Mcp,
        RateLimitTier::Service,
        RateLimitTier::Anon,
    ];

    for tier in tiers {
        let params = SessionParams {
            user_id: &user_id,
            session_id: &session_id,
            email: "test@example.com",
            duration: Duration::hours(1),
            user_type: UserType::User,
            permissions: vec![Permission::User],
            roles: vec!["user".to_string()],
            rate_limit_tier: tier,
        };

        let result = generator.generate(&params);
        assert!(result.is_ok(), "Failed for rate limit tier {:?}", tier);
    }
}

#[test]
fn test_session_params_all_user_types() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let user_types = [UserType::User, UserType::Admin, UserType::Anon];

    for user_type in user_types {
        let params = SessionParams {
            user_id: &user_id,
            session_id: &session_id,
            email: "test@example.com",
            duration: Duration::hours(1),
            user_type,
            permissions: vec![Permission::User],
            roles: vec!["user".to_string()],
            rate_limit_tier: RateLimitTier::User,
        };

        let result = generator.generate(&params);
        assert!(result.is_ok(), "Failed for user type {:?}", user_type);
    }
}

#[test]
fn test_generate_session_token_special_email_characters() {
    let generator = create_test_generator();
    let user_id = UserId::new("user".to_string());
    let session_id = SessionId::new("session".to_string());

    let params = SessionParams {
        user_id: &user_id,
        session_id: &session_id,
        email: "user+tag@sub.domain.example.com",
        duration: Duration::hours(1),
        user_type: UserType::User,
        permissions: vec![Permission::User],
        roles: vec!["user".to_string()],
        rate_limit_tier: RateLimitTier::User,
    };

    let result = generator.generate(&params);
    assert!(result.is_ok());
}

#[test]
fn test_generate_session_token_uuid_ids() {
    let generator = create_test_generator();
    let user_id = UserId::new("550e8400-e29b-41d4-a716-446655440000".to_string());
    let session_id = SessionId::new("6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string());
    let params = create_test_params(&user_id, &session_id);

    let result = generator.generate(&params);
    assert!(result.is_ok());
}
