//! Unit tests for JWT token handling
//!
//! Tests cover:
//! - decode_expiry function with valid and invalid tokens
//! - is_expired function with expired and valid tokens
//! - expires_within function with various durations

use base64::prelude::*;
use chrono::{Duration, Utc};
use systemprompt_cloud::jwt::{decode_expiry, expires_within, is_expired};

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a valid JWT token with a specific expiry timestamp
fn create_test_token(exp: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("test_signature");
    format!("{}.{}.{}", header, payload, signature)
}

/// Creates a token that expires at a specific offset from now
fn create_token_with_offset(seconds_from_now: i64) -> String {
    let exp = Utc::now().timestamp() + seconds_from_now;
    create_test_token(exp)
}

// ============================================================================
// decode_expiry Tests
// ============================================================================

#[test]
fn test_decode_expiry_valid_token() {
    let expected_exp = 1735689600; // 2025-01-01 00:00:00 UTC
    let token = create_test_token(expected_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_exp);
}

#[test]
fn test_decode_expiry_future_timestamp() {
    let future_exp = Utc::now().timestamp() + 3600; // 1 hour from now
    let token = create_test_token(future_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), future_exp);
}

#[test]
fn test_decode_expiry_past_timestamp() {
    let past_exp = Utc::now().timestamp() - 3600; // 1 hour ago
    let token = create_test_token(past_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), past_exp);
}

#[test]
fn test_decode_expiry_invalid_format_no_dots() {
    let result = decode_expiry("invalid_token_without_dots");
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_format_one_dot() {
    let result = decode_expiry("header.payload");
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_format_too_many_dots() {
    let result = decode_expiry("part1.part2.part3.part4");
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_base64_payload() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let token = format!("{}.not_valid_base64!!!.signature", header);

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_json_payload() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode("not valid json");
    let token = format!("{}.{}.signature", header, payload);

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_missing_exp_claim() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(r#"{"sub":"user123"}"#);
    let token = format!("{}.{}.signature", header, payload);

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_empty_token() {
    let result = decode_expiry("");
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_empty_parts() {
    let result = decode_expiry("..");
    assert!(result.is_err());
}

// ============================================================================
// is_expired Tests
// ============================================================================

#[test]
fn test_is_expired_with_expired_token() {
    let token = create_token_with_offset(-3600); // Expired 1 hour ago
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_with_valid_token() {
    let token = create_token_with_offset(3600); // Expires in 1 hour
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_just_expired() {
    let token = create_token_with_offset(-1); // Expired 1 second ago
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_about_to_expire() {
    let token = create_token_with_offset(1); // Expires in 1 second
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_with_invalid_token() {
    // Invalid tokens should be treated as expired for safety
    assert!(is_expired("invalid.token.here"));
}

#[test]
fn test_is_expired_with_malformed_token() {
    assert!(is_expired("not_a_jwt"));
}

#[test]
fn test_is_expired_far_future() {
    let token = create_token_with_offset(86400 * 365); // Expires in 1 year
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_far_past() {
    let token = create_token_with_offset(-86400 * 365); // Expired 1 year ago
    assert!(is_expired(&token));
}

// ============================================================================
// expires_within Tests
// ============================================================================

#[test]
fn test_expires_within_token_expiring_soon() {
    let token = create_token_with_offset(1800); // Expires in 30 minutes
    assert!(expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_token_not_expiring_soon() {
    let token = create_token_with_offset(7200); // Expires in 2 hours
    assert!(!expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_already_expired() {
    let token = create_token_with_offset(-3600); // Already expired
    assert!(expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_exactly_at_threshold() {
    let token = create_token_with_offset(3600); // Expires in exactly 1 hour
    // At threshold should be considered "within" since exp < threshold
    assert!(!expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_zero_duration() {
    let token = create_token_with_offset(60); // Expires in 1 minute
    assert!(!expires_within(&token, Duration::zero()));
}

#[test]
fn test_expires_within_negative_duration() {
    let token = create_token_with_offset(3600); // Expires in 1 hour
    // Negative duration means checking if expired in the past
    assert!(!expires_within(&token, Duration::hours(-1)));
}

#[test]
fn test_expires_within_with_invalid_token() {
    // Invalid tokens should be treated as expiring within any duration
    assert!(expires_within("invalid.token", Duration::hours(24)));
}

#[test]
fn test_expires_within_various_durations() {
    let token = create_token_with_offset(1800); // Expires in 30 minutes

    assert!(expires_within(&token, Duration::hours(1))); // Within 1 hour
    assert!(expires_within(&token, Duration::minutes(45))); // Within 45 minutes
    assert!(!expires_within(&token, Duration::minutes(15))); // Not within 15 minutes
    assert!(!expires_within(&token, Duration::minutes(10))); // Not within 10 minutes
}

#[test]
fn test_expires_within_days() {
    let token = create_token_with_offset(86400 * 5); // Expires in 5 days

    assert!(expires_within(&token, Duration::days(7))); // Within 7 days
    assert!(!expires_within(&token, Duration::days(3))); // Not within 3 days
}
