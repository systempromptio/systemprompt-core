//! Tests for JWT validation

use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::validate_jwt_token;

const TEST_SECRET: &str = "test_secret_key_for_jwt_validation_tests_12345";
const TEST_ISSUER: &str = "https://test.systemprompt.io";

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
        sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        iat: now,
        exp: now + exp_offset_secs,
        iss: issuer.to_string(),
        aud: audiences.iter().map(|s| s.to_string()).collect(),
        jti: "test-jti-123".to_string(),
        scope: "user".to_string(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        user_type: "user".to_string(),
        roles: vec!["user".to_string()],
        token_type: "Bearer".to_string(),
        auth_time: now,
    }
}

fn create_test_token(claims: &TestClaims, secret: &str) -> String {
    let header = Header::new(Algorithm::HS256);
    encode(&header, claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
}

// ============================================================================
// Valid Token Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_success() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_ok());
    let validated_claims = result.unwrap();
    assert_eq!(validated_claims.sub, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(validated_claims.username, "testuser");
    assert_eq!(validated_claims.email, "test@example.com");
}

#[test]
fn test_validate_jwt_token_multiple_audiences() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["api", "web"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(
        &token,
        TEST_SECRET,
        TEST_ISSUER,
        &[JwtAudience::Api, JwtAudience::Web],
    );

    assert!(result.is_ok());
}

#[test]
fn test_validate_jwt_token_with_mcp_audience() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["mcp"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Mcp]);

    assert!(result.is_ok());
}

#[test]
fn test_validate_jwt_token_with_a2a_audience() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["a2a"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::A2a]);

    assert!(result.is_ok());
}

// ============================================================================
// Expired Token Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_expired() {
    let claims = create_test_claims(-3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string().to_lowercase();
    assert!(err_msg.contains("expired") || err_msg.contains("exp"));
}

#[test]
fn test_validate_jwt_token_just_expired() {
    let claims = create_test_claims(-1, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
}

// ============================================================================
// Invalid Secret Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_wrong_secret() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, "wrong_secret", TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("validation failed"));
}

// ============================================================================
// Invalid Issuer Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_wrong_issuer() {
    let claims = create_test_claims(3600, "https://wrong-issuer.com", &["api"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("validation failed"));
}

// ============================================================================
// Invalid Audience Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_wrong_audience() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["wrong_audience"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("validation failed"));
}

#[test]
fn test_validate_jwt_token_missing_required_audience() {
    let claims = create_test_claims(3600, TEST_ISSUER, &["mcp"]);
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
}

// ============================================================================
// Malformed Token Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_malformed() {
    let result = validate_jwt_token(
        "not.a.valid.jwt",
        TEST_SECRET,
        TEST_ISSUER,
        &[JwtAudience::Api],
    );

    assert!(result.is_err());
}

#[test]
fn test_validate_jwt_token_empty() {
    let result = validate_jwt_token("", TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_err());
}

#[test]
fn test_validate_jwt_token_random_string() {
    let result = validate_jwt_token(
        "random_gibberish_string",
        TEST_SECRET,
        TEST_ISSUER,
        &[JwtAudience::Api],
    );

    assert!(result.is_err());
}

#[test]
fn test_validate_jwt_token_base64_but_not_jwt() {
    let result = validate_jwt_token(
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo=",
        TEST_SECRET,
        TEST_ISSUER,
        &[JwtAudience::Api],
    );

    assert!(result.is_err());
}

// ============================================================================
// Claims Validation Tests
// ============================================================================

#[test]
fn test_validate_jwt_token_extracts_correct_subject() {
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.sub = "user-id-12345".to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap().sub, "user-id-12345");
}

#[test]
fn test_validate_jwt_token_extracts_username_and_email() {
    let mut claims = create_test_claims(3600, TEST_ISSUER, &["api"]);
    claims.username = "john_doe".to_string();
    claims.email = "john@example.com".to_string();
    let token = create_test_token(&claims, TEST_SECRET);

    let result = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);

    assert!(result.is_ok());
    let validated = result.unwrap();
    assert_eq!(validated.username, "john_doe");
    assert_eq!(validated.email, "john@example.com");
}
