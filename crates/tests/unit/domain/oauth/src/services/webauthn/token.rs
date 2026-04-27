//! Tests for WebAuthn setup token generation and validation

use systemprompt_oauth::services::webauthn::{
    generate_setup_token, hash_token, validate_token_format,
};

const TOKEN_PREFIX: &str = "sp_wst_";

// ============================================================================
// Token Generation Tests
// ============================================================================

#[test]
fn test_generate_setup_token_has_prefix() {
    let (token, _hash) = generate_setup_token();

    assert!(token.starts_with(TOKEN_PREFIX));
}

#[test]
fn test_generate_setup_token_unique() {
    let (token_a, _) = generate_setup_token();
    let (token_b, _) = generate_setup_token();

    assert_ne!(token_a, token_b);
}

#[test]
fn test_generate_setup_token_hash_matches() {
    let (token, hash) = generate_setup_token();

    assert_eq!(hash_token(&token), hash);
}

// ============================================================================
// Hash Tests
// ============================================================================

#[test]
fn test_hash_token_deterministic() {
    let input = "sp_wst_test_input_value";

    let hash_a = hash_token(input);
    let hash_b = hash_token(input);

    assert_eq!(hash_a, hash_b);
}

#[test]
fn test_hash_token_different_inputs() {
    let hash_a = hash_token("sp_wst_input_one");
    let hash_b = hash_token("sp_wst_input_two");

    assert_ne!(hash_a, hash_b);
}

// ============================================================================
// Format Validation Tests
// ============================================================================

#[test]
fn test_validate_token_format_valid() {
    let (token, _) = generate_setup_token();

    assert!(validate_token_format(&token).is_ok());
}

#[test]
fn test_validate_token_format_missing_prefix() {
    let result = validate_token_format("invalid_no_prefix_token");

    assert!(result.is_err());
}

#[test]
fn test_validate_token_format_invalid_encoding() {
    let result = validate_token_format("sp_wst_!!!");

    assert!(result.is_err());
}
