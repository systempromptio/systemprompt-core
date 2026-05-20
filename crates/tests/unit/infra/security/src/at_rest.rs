//! Unit coverage for the HMAC-SHA-256 at-rest helper.

use systemprompt_security::{hmac_sha256, hmac_sha256_hex};

const PEPPER_A: &[u8] = b"pepper_alpha_a_a_a_a_a_a_a_a_a_a_a_a_a";
const PEPPER_B: &[u8] = b"pepper_beta_b_b_b_b_b_b_b_b_b_b_b_b_b_b";

#[test]
fn deterministic_under_same_pepper() {
    let a = hmac_sha256(PEPPER_A, b"refresh_token_42");
    let b = hmac_sha256(PEPPER_A, b"refresh_token_42");
    assert_eq!(a, b, "same pepper + same input must hash identically");
}

#[test]
fn diverges_across_peppers() {
    let a = hmac_sha256(PEPPER_A, b"refresh_token_42");
    let b = hmac_sha256(PEPPER_B, b"refresh_token_42");
    assert_ne!(
        a, b,
        "rotating pepper must break collision with prior digest"
    );
}

#[test]
fn diverges_across_inputs() {
    let a = hmac_sha256(PEPPER_A, b"refresh_token_42");
    let b = hmac_sha256(PEPPER_A, b"refresh_token_43");
    assert_ne!(a, b, "distinct inputs must yield distinct digests");
}

#[test]
fn hex_form_is_lowercase_64_chars() {
    let hex = hmac_sha256_hex(PEPPER_A, b"refresh_token_42");
    assert_eq!(hex.len(), 64, "SHA-256 hex digest is 64 characters");
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
        "hex digest must be lowercase ASCII"
    );
}
