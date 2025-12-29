//! Tests for token generation services

use systemprompt_core_oauth::services::{
    generate_access_token_jti, generate_client_secret, generate_secure_token, hash_client_secret,
    verify_client_secret, JwtConfig,
};
use systemprompt_models::auth::{JwtAudience, Permission};

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
    assert!(parsed.is_ok());
}

// ============================================================================
// hash_client_secret and verify_client_secret Tests
// ============================================================================

#[test]
fn test_hash_client_secret_success() {
    let secret = "my_test_secret_123";
    let result = hash_client_secret(secret);
    assert!(result.is_ok());
    let hash = result.unwrap();
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
    assert!(result.is_ok());
}

#[test]
fn test_verify_client_secret_correct() {
    let secret = "correct_secret";
    let hash = hash_client_secret(secret).unwrap();
    let result = verify_client_secret(secret, &hash);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_verify_client_secret_incorrect() {
    let secret = "original_secret";
    let wrong_secret = "wrong_secret";
    let hash = hash_client_secret(secret).unwrap();
    let result = verify_client_secret(wrong_secret, &hash);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[test]
fn test_verify_client_secret_invalid_hash() {
    let secret = "some_secret";
    let invalid_hash = "not_a_valid_bcrypt_hash";
    let result = verify_client_secret(secret, invalid_hash);
    assert!(result.is_err());
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
    assert!(result.is_ok());
    let hash = result.unwrap();
    let verified = verify_client_secret(secret, &hash).unwrap();
    assert!(verified);
}

#[test]
fn test_hash_client_secret_unicode() {
    let secret = "秘密🔐パスワード";
    let result = hash_client_secret(secret);
    assert!(result.is_ok());
    let hash = result.unwrap();
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
    };

    assert!(config.expires_in_hours.is_none());
}

#[test]
fn test_jwt_config_clone() {
    let config = JwtConfig::default();
    let cloned = config.clone();
    assert_eq!(config.permissions, cloned.permissions);
    assert_eq!(config.expires_in_hours, cloned.expires_in_hours);
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
