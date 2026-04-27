//! Tests for token generation services

use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_oauth::services::{
    JwtConfig, generate_access_token_jti, generate_client_secret, generate_secure_token,
    hash_client_secret, verify_client_secret,
};

// ============================================================================
// generate_secure_token Tests
// ============================================================================

#[test]
fn test_generate_secure_token_with_prefix() {
    let token = generate_secure_token("auth");
    assert!(token.starts_with("auth_"));
}

#[test]
fn test_generate_secure_token_length() {
    let token = generate_secure_token("test");
    // Format: prefix_<32 alphanumeric chars>
    // "test_" = 5 chars + 32 = 37 total
    assert_eq!(token.len(), 37);
}

#[test]
fn test_generate_secure_token_alphanumeric() {
    let token = generate_secure_token("tok");
    let suffix = token.strip_prefix("tok_").unwrap();
    assert!(suffix.chars().all(|c| c.is_alphanumeric()));
}

#[test]
fn test_generate_secure_token_unique() {
    let token1 = generate_secure_token("uniq");
    let token2 = generate_secure_token("uniq");
    assert_ne!(token1, token2);
}

#[test]
fn test_generate_secure_token_empty_prefix() {
    let token = generate_secure_token("");
    assert!(token.starts_with("_"));
    assert_eq!(token.len(), 33); // "_" + 32 chars
}

#[test]
fn test_generate_secure_token_long_prefix() {
    let token = generate_secure_token("very_long_prefix_for_testing");
    assert!(token.starts_with("very_long_prefix_for_testing_"));
}

// ============================================================================
// generate_client_secret Tests
// ============================================================================

#[test]
fn test_generate_client_secret_prefix() {
    let secret = generate_client_secret();
    assert!(secret.starts_with("secret_"));
}

#[test]
fn test_generate_client_secret_length() {
    let secret = generate_client_secret();
    // Format: "secret_" (7 chars) + 64 alphanumeric chars = 71 total
    assert_eq!(secret.len(), 71);
}

#[test]
fn test_generate_client_secret_alphanumeric() {
    let secret = generate_client_secret();
    let suffix = secret.strip_prefix("secret_").unwrap();
    assert!(suffix.chars().all(|c| c.is_alphanumeric()));
}

#[test]
fn test_generate_client_secret_unique() {
    let secret1 = generate_client_secret();
    let secret2 = generate_client_secret();
    assert_ne!(secret1, secret2);
}

// ============================================================================
// generate_access_token_jti Tests
// ============================================================================

#[test]
fn test_generate_access_token_jti_is_uuid() {
    let jti = generate_access_token_jti();
    // UUID v4 format: 8-4-4-4-12 = 36 chars with hyphens
    assert_eq!(jti.len(), 36);
    assert!(jti.contains('-'));
}

#[test]
fn test_generate_access_token_jti_unique() {
    let jti1 = generate_access_token_jti();
    let jti2 = generate_access_token_jti();
    assert_ne!(jti1, jti2);
}

#[test]
fn test_generate_access_token_jti_valid_uuid_format() {
    let jti = generate_access_token_jti();
    let parts: Vec<&str> = jti.split('-').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 4);
    assert_eq!(parts[2].len(), 4);
    assert_eq!(parts[3].len(), 4);
    assert_eq!(parts[4].len(), 12);
}

#[test]
fn test_generate_access_token_jti_parseable_as_uuid() {
    let jti = generate_access_token_jti();
    let parsed = uuid::Uuid::parse_str(&jti);
    parsed.expect("expected success");
}

// ============================================================================
// hash_client_secret and verify_client_secret Tests
// ============================================================================

#[test]
fn test_hash_client_secret_success() {
    let secret = "my_test_secret_123";
    let result = hash_client_secret(secret);
    let hash = result.expect("expected success");
    assert!(!hash.is_empty());
    // bcrypt hashes start with "$2b$" or "$2a$"
    assert!(hash.starts_with("$2"));
}

#[test]
fn test_hash_client_secret_different_hashes() {
    let secret = "same_secret";
    let hash1 = hash_client_secret(secret).unwrap();
    let hash2 = hash_client_secret(secret).unwrap();
    // Same secret should produce different hashes due to salt
    assert_ne!(hash1, hash2);
}

#[test]
fn test_hash_client_secret_empty_secret() {
    let secret = "";
    let result = hash_client_secret(secret);
    result.expect("expected success");
}

#[test]
fn test_verify_client_secret_correct() {
    let secret = "correct_secret";
    let hash = hash_client_secret(secret).unwrap();
    let result = verify_client_secret(secret, &hash);
    let val = result.expect("expected success");
    assert!(val);
}

#[test]
fn test_verify_client_secret_incorrect() {
    let secret = "original_secret";
    let wrong_secret = "wrong_secret";
    let hash = hash_client_secret(secret).unwrap();
    let result = verify_client_secret(wrong_secret, &hash);
    let val = result.expect("expected success");
    assert!(!val);
}

#[test]
fn test_verify_client_secret_invalid_hash() {
    let secret = "some_secret";
    let invalid_hash = "not_a_valid_bcrypt_hash";
    let result = verify_client_secret(secret, invalid_hash);
    result.unwrap_err();
}

#[test]
fn test_hash_and_verify_generated_secret() {
    let secret = generate_client_secret();
    let hash = hash_client_secret(&secret).unwrap();
    let verified = verify_client_secret(&secret, &hash).unwrap();
    assert!(verified);
}

#[test]
fn test_verify_client_secret_case_sensitive() {
    let secret = "CaseSensitiveSecret";
    let hash = hash_client_secret(secret).unwrap();

    let same_case = verify_client_secret(secret, &hash).unwrap();
    assert!(same_case);

    let different_case = verify_client_secret("casesensitivesecret", &hash).unwrap();
    assert!(!different_case);
}

#[test]
fn test_hash_client_secret_special_characters() {
    let secret = "secret!@#$%^&*()_+-=[]{}|;':\",./<>?";
    let result = hash_client_secret(secret);
    let hash = result.expect("expected success");
    let verified = verify_client_secret(secret, &hash).unwrap();
    assert!(verified);
}

#[test]
fn test_hash_client_secret_unicode() {
    let secret = "秘密🔐パスワード";
    let result = hash_client_secret(secret);
    let hash = result.expect("expected success");
    let verified = verify_client_secret(secret, &hash).unwrap();
    assert!(verified);
}

// ============================================================================
// JwtConfig Tests
// ============================================================================

#[test]
fn test_jwt_config_default() {
    let config = JwtConfig::default();
    assert_eq!(config.permissions, vec![Permission::User]);
    assert_eq!(config.audience, JwtAudience::standard());
    assert_eq!(config.expires_in_hours, Some(24));
}

#[test]
fn test_jwt_config_custom() {
    let config = JwtConfig {
        permissions: vec![Permission::Admin, Permission::User],
        audience: vec![JwtAudience::Api],
        expires_in_hours: Some(48),
        resource: None,
    };

    assert_eq!(config.permissions.len(), 2);
    assert!(config.permissions.contains(&Permission::Admin));
    assert_eq!(config.audience, vec![JwtAudience::Api]);
    assert_eq!(config.expires_in_hours, Some(48));
}

#[test]
fn test_jwt_config_no_expiry() {
    let config = JwtConfig {
        permissions: vec![Permission::User],
        audience: JwtAudience::standard(),
        expires_in_hours: None,
        resource: None,
    };

    assert!(config.expires_in_hours.is_none());
}

#[test]
fn test_jwt_config_debug() {
    let config = JwtConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("JwtConfig"));
    assert!(debug_str.contains("permissions"));
}

#[test]
fn test_jwt_config_serialize() {
    let config = JwtConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("permissions"));
    assert!(json.contains("audience"));
    assert!(json.contains("expires_in_hours"));
}

#[test]
fn test_jwt_config_deserialize() {
    let json = r#"{
        "permissions": ["admin"],
        "audience": ["api"],
        "expires_in_hours": 72
    }"#;

    let config: JwtConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.permissions, vec![Permission::Admin]);
    assert_eq!(config.audience, vec![JwtAudience::Api]);
    assert_eq!(config.expires_in_hours, Some(72));
}

// ============================================================================
// generate_anonymous_jwt_with_expiry Tests
// ============================================================================

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::auth::{JwtClaims, UserType};
use systemprompt_oauth::services::{
    JwtSigningParams, generate_admin_jwt_with_expiry, generate_anonymous_jwt_with_expiry,
};

fn test_signing_params() -> JwtSigningParams<'static> {
    JwtSigningParams {
        secret: "test-secret-key-for-unit-tests-only",
        issuer: "test-issuer",
    }
}

fn decode_token(token: &str, secret: &str) -> JwtClaims {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;
    validation.validate_aud = false;
    validation.set_required_spec_claims::<String>(&[]);
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .expect("expected successful decode")
    .claims
}

#[test]
fn test_generate_anonymous_jwt_with_expiry_produces_valid_jwt() {
    let signing = test_signing_params();
    let client_id = ClientId::new("test-client-id");
    let token = generate_anonymous_jwt_with_expiry(
        &UserId::new("anon-user-1"),
        &SessionId::new("session-1"),
        &client_id,
        &signing,
        3600,
    )
    .unwrap();

    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(!parts[0].is_empty());
    assert!(!parts[1].is_empty());
    assert!(!parts[2].is_empty());
}

#[test]
fn test_generate_anonymous_jwt_with_expiry_standard_duration() {
    let signing = test_signing_params();
    let client_id = ClientId::new("client-abc");
    let token = generate_anonymous_jwt_with_expiry(
        &UserId::new("anon-user-2"),
        &SessionId::new("session-2"),
        &client_id,
        &signing,
        7200,
    )
    .unwrap();

    let claims = decode_token(&token, signing.secret);
    assert_eq!(claims.sub, "anon-user-2");
    assert_eq!(claims.iss, "test-issuer");
    assert_eq!(claims.user_type, UserType::Anon);
    assert_eq!(claims.scope, vec![Permission::Anonymous]);
}

#[test]
fn test_generate_anonymous_jwt_with_expiry_zero_seconds() {
    let signing = test_signing_params();
    let client_id = ClientId::new("client-zero");
    let result = generate_anonymous_jwt_with_expiry(
        &UserId::new("anon-user-3"),
        &SessionId::new("session-3"),
        &client_id,
        &signing,
        0,
    );

    let token = result.expect("expected success with zero seconds");
    let claims = decode_token(&token, signing.secret);
    assert!(claims.exp <= claims.iat);
}

#[test]
fn test_generate_anonymous_jwt_with_expiry_claims_contain_client_id() {
    let signing = test_signing_params();
    let client_id = ClientId::new("my-client-123");
    let token = generate_anonymous_jwt_with_expiry(
        &UserId::new("anon-user-4"),
        &SessionId::new("session-4"),
        &client_id,
        &signing,
        3600,
    )
    .unwrap();

    let claims = decode_token(&token, signing.secret);
    assert_eq!(claims.client_id, Some("my-client-123".to_string()));
    assert_eq!(claims.session_id, Some("session-4".to_string()));
    assert_eq!(claims.username, "anon-user-4");
    assert_eq!(claims.email, "anon-user-4");
}

// ============================================================================
// generate_admin_jwt_with_expiry Tests
// ============================================================================

#[test]
fn test_generate_admin_jwt_with_expiry_produces_valid_jwt() {
    let signing = test_signing_params();
    let client_id = ClientId::new("admin-client");
    let token = generate_admin_jwt_with_expiry(
        &UserId::new("admin-user-1"),
        &SessionId::new("admin-session-1"),
        "admin@example.com",
        &client_id,
        &signing,
        3600,
    )
    .unwrap();

    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
}

#[test]
fn test_generate_admin_jwt_with_expiry_standard_duration() {
    let signing = test_signing_params();
    let client_id = ClientId::new("admin-client-2");
    let token = generate_admin_jwt_with_expiry(
        &UserId::new("admin-user-2"),
        &SessionId::new("admin-session-2"),
        "admin2@example.com",
        &client_id,
        &signing,
        7200,
    )
    .unwrap();

    let claims = decode_token(&token, signing.secret);
    assert_eq!(claims.sub, "admin-user-2");
    assert_eq!(claims.iss, "test-issuer");
    assert_eq!(claims.user_type, UserType::Admin);
    assert_eq!(claims.scope, vec![Permission::Admin]);
}

#[test]
fn test_generate_admin_jwt_with_expiry_claims_contain_email_and_roles() {
    let signing = test_signing_params();
    let client_id = ClientId::new("admin-client-3");
    let token = generate_admin_jwt_with_expiry(
        &UserId::new("admin-user-3"),
        &SessionId::new("admin-session-3"),
        "super@example.com",
        &client_id,
        &signing,
        3600,
    )
    .unwrap();

    let claims = decode_token(&token, signing.secret);
    assert_eq!(claims.email, "super@example.com");
    assert_eq!(claims.username, "super@example.com");
    assert_eq!(claims.client_id, Some("admin-client-3".to_string()));
    assert_eq!(claims.session_id, Some("admin-session-3".to_string()));
    assert!(claims.roles.contains(&"admin".to_string()));
    assert!(claims.roles.contains(&"user".to_string()));
}

#[test]
fn test_generate_admin_jwt_with_expiry_zero_seconds() {
    let signing = test_signing_params();
    let client_id = ClientId::new("admin-client-zero");
    let result = generate_admin_jwt_with_expiry(
        &UserId::new("admin-user-4"),
        &SessionId::new("admin-session-4"),
        "admin4@example.com",
        &client_id,
        &signing,
        0,
    );

    let token = result.expect("expected success with zero seconds");
    let claims = decode_token(&token, signing.secret);
    assert!(claims.exp <= claims.iat);
}
