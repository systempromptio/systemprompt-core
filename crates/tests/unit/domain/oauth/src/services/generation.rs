//! Tests for token generation services

use base64::Engine;
use jsonwebtoken::decode_header;
use systemprompt_identifiers::ClientId;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_oauth::services::generation::{IdJagGrant, mint_id_jag};
use systemprompt_oauth::services::validation::id_jag::{ID_JAG_TYP, IdJagClaims};
use systemprompt_oauth::services::{
    JwtConfig, generate_access_token_jti, generate_client_secret, generate_secure_token,
    hash_client_secret, verify_client_secret,
};
use systemprompt_test_fixtures::install_test_signing_key;

#[test]
fn test_generate_secure_token_with_prefix() {
    let token = generate_secure_token("auth");
    assert!(token.starts_with("auth_"));
}

#[test]
fn test_generate_secure_token_length() {
    let token = generate_secure_token("test");
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

#[test]
fn test_generate_client_secret_prefix() {
    let secret = generate_client_secret();
    assert!(secret.starts_with("secret_"));
}

#[test]
fn test_generate_client_secret_length() {
    let secret = generate_client_secret();
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

#[test]
fn test_generate_access_token_jti_is_uuid() {
    let jti = generate_access_token_jti();
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

#[test]
fn test_hash_client_secret_success() {
    let secret = "my_test_secret_123";
    let result = hash_client_secret(secret);
    let hash = result.expect("expected success");
    assert!(!hash.is_empty());
    assert!(hash.starts_with("$2"));
}

#[test]
fn test_hash_client_secret_different_hashes() {
    let secret = "same_secret";
    let hash1 = hash_client_secret(secret).unwrap();
    let hash2 = hash_client_secret(secret).unwrap();
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
        plugin_id: None,
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
        plugin_id: None,
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

// Base64url-decode the JWT payload directly. These tests cover the minting
// logic (typ header + claim shape), not the signing authority, so no signature
// verification is needed.
fn id_jag_claims(token: &str) -> IdJagClaims {
    let payload = token
        .split('.')
        .nth(1)
        .expect("token has a payload segment");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("payload is base64url");
    serde_json::from_slice(&bytes).expect("payload deserializes to IdJagClaims")
}

#[test]
fn mints_id_jag_with_correct_typ_and_claims() {
    install_test_signing_key();
    let client_id = ClientId::new("client-a");
    let token = mint_id_jag(&IdJagGrant {
        sub: "user-42",
        email: Some("user@example.com"),
        aud: "https://core.example",
        client_id: &client_id,
        scope: Some("user mcp"),
        ttl_secs: 300,
        issuer: "https://core.example",
    })
    .expect("mint");

    assert_eq!(
        decode_header(&token).expect("header").typ.as_deref(),
        Some(ID_JAG_TYP)
    );
    let claims = id_jag_claims(&token);
    assert_eq!(claims.sub, "user-42");
    assert_eq!(claims.aud, "https://core.example");
    assert_eq!(
        claims.bound_client().map(ClientId::as_str),
        Some("client-a")
    );
    assert_eq!(claims.email.as_deref(), Some("user@example.com"));
    assert_eq!(claims.scope.as_deref(), Some("user mcp"));
    assert_eq!(claims.exp - claims.iat, 300);
}

#[test]
fn defaults_optional_claims_to_none() {
    install_test_signing_key();
    let client_id = ClientId::new("client-b");
    let token = mint_id_jag(&IdJagGrant {
        sub: "user-1",
        email: None,
        aud: "https://rs.example",
        client_id: &client_id,
        scope: None,
        ttl_secs: 120,
        issuer: "https://core.example",
    })
    .expect("mint");
    let claims = id_jag_claims(&token);
    assert!(claims.email.is_none());
    assert!(claims.scope.is_none());
    assert_eq!(claims.iss, "https://core.example");
}
