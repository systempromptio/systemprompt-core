//! Tests for JwtTokenValidator

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::TokenValidator;
use systemprompt_oauth::services::JwtTokenValidator;

const TEST_SECRET: &str = "webauthn_jwt_test_secret_key_long_enough_12345";
const TEST_ISSUER: &str = "https://test.systemprompt.io";
const TEST_USER_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_USERNAME: &str = "testuser";
const TEST_EMAIL: &str = "test@example.com";

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: Vec<String>,
    jti: String,
    scope: String,
    username: String,
    email: String,
    user_type: String,
    roles: Vec<String>,
    token_type: String,
    auth_time: i64,
}

fn create_test_claims(exp_offset_secs: i64, issuer: &str, audiences: &[&str]) -> TestClaims {
    let now = Utc::now().timestamp();
    TestClaims {
        sub: TEST_USER_UUID.to_string(),
        iat: now,
        exp: now + exp_offset_secs,
        iss: issuer.to_string(),
        aud: audiences.iter().map(|s| s.to_string()).collect(),
        jti: "test-jti-webauthn-123".to_string(),
        scope: "user".to_string(),
        username: TEST_USERNAME.to_string(),
        email: TEST_EMAIL.to_string(),
        user_type: "user".to_string(),
        roles: vec!["user".to_string()],
        token_type: "Bearer".to_string(),
        auth_time: now,
    }
}

fn create_test_token(claims: &TestClaims, secret: &str) -> String {
    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

fn create_validator() -> JwtTokenValidator {
    JwtTokenValidator::new(
        TEST_SECRET.to_string(),
        TEST_ISSUER.to_string(),
        vec![JwtAudience::Api],
    )
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_jwt_token_validator_new() {
    let validator = JwtTokenValidator::new(
        "secret".to_string(),
        "issuer".to_string(),
        vec![JwtAudience::Api, JwtAudience::Web],
    );

    let debug_output = format!("{validator:?}");
    assert!(debug_output.contains("JwtTokenValidator"));
}

// ============================================================================
// Successful Validation Tests
// ============================================================================

#[tokio::test]
async fn test_jwt_token_validator_validate_success() {
    let validator = create_validator();
    let claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    let user = result.expect("expected successful validation");
    assert_eq!(user.id.to_string(), TEST_USER_UUID);
}

#[tokio::test]
async fn test_jwt_token_validator_validate_extracts_uuid() {
    let validator = create_validator();
    let custom_uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.sub = custom_uuid.to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    let user = result.expect("expected successful validation");
    assert_eq!(user.id.to_string(), custom_uuid);
}

#[tokio::test]
async fn test_jwt_token_validator_validate_extracts_username() {
    let validator = create_validator();
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.username = "custom_user".to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    let user = result.expect("expected successful validation");
    assert_eq!(user.username, "custom_user");
}

#[tokio::test]
async fn test_jwt_token_validator_validate_extracts_email() {
    let validator = create_validator();
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.email = "custom@domain.com".to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    let user = result.expect("expected successful validation");
    assert_eq!(user.email, "custom@domain.com");
}

// ============================================================================
// Error Validation Tests
// ============================================================================

#[tokio::test]
async fn test_jwt_token_validator_validate_expired() {
    let validator = create_validator();
    let claims = create_test_claims(-3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_jwt_token_validator_validate_wrong_secret() {
    let validator = create_validator();
    let claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, "wrong_secret_key_that_does_not_match");

    let result = validator.validate_token(&token).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_jwt_token_validator_validate_malformed() {
    let validator = create_validator();

    let result = validator.validate_token("not.a.valid.jwt").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_jwt_token_validator_validate_invalid_uuid_subject() {
    let validator = create_validator();
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.sub = "not-a-valid-uuid".to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validator.validate_token(&token).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_jwt_token_validator_validate_empty_token() {
    let validator = create_validator();

    let result = validator.validate_token("").await;

    assert!(result.is_err());
}
