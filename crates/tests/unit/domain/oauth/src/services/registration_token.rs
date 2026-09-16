//! RFC 7592 registration access token generation and verification.

use systemprompt_oauth::services::{
    REGISTRATION_TOKEN_PREFIX, generate_registration_token, hash_registration_token,
    verify_registration_token,
};

#[test]
fn generated_tokens_carry_the_prefix_and_are_unique() {
    let a = generate_registration_token();
    let b = generate_registration_token();
    assert!(a.starts_with(REGISTRATION_TOKEN_PREFIX));
    assert!(a.len() > REGISTRATION_TOKEN_PREFIX.len() + 40, "{a}");
    assert_ne!(a, b);
}

#[test]
fn the_stored_hash_is_not_the_token() {
    let token = generate_registration_token();
    let hash = hash_registration_token(&token);
    assert_ne!(hash, token);
    assert_eq!(hash.len(), 64, "sha-256 hex digest");
}

#[test]
fn verification_matches_only_the_issued_token() {
    let token = generate_registration_token();
    let hash = hash_registration_token(&token);
    assert!(verify_registration_token(&token, Some(&hash)));
    assert!(!verify_registration_token(
        &generate_registration_token(),
        Some(&hash)
    ));
}

#[test]
fn a_client_without_a_stored_hash_never_verifies() {
    let token = generate_registration_token();
    assert!(!verify_registration_token(&token, None));
    assert!(!verify_registration_token(&token, Some("")));
}
