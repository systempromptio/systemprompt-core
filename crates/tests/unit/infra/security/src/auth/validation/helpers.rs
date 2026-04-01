//! Shared test helpers for auth validation tests

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::AuthValidationService;

pub fn create_test_service() -> AuthValidationService {
    AuthValidationService::new(
        "test_secret_key".to_string(),
        "test_issuer".to_string(),
        JwtAudience::standard(),
    )
}

pub fn create_valid_jwt(secret: &str, issuer: &str, session_id: Option<String>) -> String {
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
        user_type: UserType::User,
        roles: vec!["user".to_string()],
        client_id: Some("test_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id,
        rate_limit_tier: Some(RateLimitTier::User),
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

pub fn create_admin_jwt(secret: &str, issuer: &str) -> String {
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
        user_type: UserType::User,
        roles: vec!["admin".to_string(), "user".to_string()],
        client_id: Some("admin_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some("admin_session".to_string()),
        rate_limit_tier: Some(RateLimitTier::Admin),
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

pub fn create_expired_jwt(secret: &str, issuer: &str) -> String {
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
        user_type: UserType::User,
        roles: vec!["user".to_string()],
        client_id: Some("test_client".to_string()),
        token_type: TokenType::Bearer,
        auth_time: (now - Duration::hours(2)).timestamp(),
        session_id: Some("session_123".to_string()),
        rate_limit_tier: Some(RateLimitTier::User),
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}
