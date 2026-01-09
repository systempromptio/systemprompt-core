//! Unit tests for JWT token handling

use base64::prelude::*;
use chrono::{Duration, Utc};
use systemprompt_cloud::auth::{decode_expiry, expires_within, is_expired};
use systemprompt_identifiers::CloudAuthToken;

fn create_test_token(exp: i64) -> CloudAuthToken {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("test_signature");
    CloudAuthToken::new(format!("{}.{}.{}", header, payload, signature))
}

fn create_token_with_offset(seconds_from_now: i64) -> CloudAuthToken {
    let exp = Utc::now().timestamp() + seconds_from_now;
    create_test_token(exp)
}

#[test]
fn test_decode_expiry_valid_token() {
    let expected_exp = 1735689600;
    let token = create_test_token(expected_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_exp);
}

#[test]
fn test_decode_expiry_future_timestamp() {
    let future_exp = Utc::now().timestamp() + 3600;
    let token = create_test_token(future_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), future_exp);
}

#[test]
fn test_decode_expiry_past_timestamp() {
    let past_exp = Utc::now().timestamp() - 3600;
    let token = create_test_token(past_exp);

    let result = decode_expiry(&token);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), past_exp);
}

#[test]
fn test_decode_expiry_invalid_format_no_dots() {
    let token = CloudAuthToken::new("invalid_token_without_dots");
    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_format_one_dot() {
    let token = CloudAuthToken::new("header.payload");
    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_format_too_many_dots() {
    let token = CloudAuthToken::new("part1.part2.part3.part4");
    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_base64_payload() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let token = CloudAuthToken::new(format!("{}.not_valid_base64!!!.signature", header));

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_invalid_json_payload() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode("not valid json");
    let token = CloudAuthToken::new(format!("{}.{}.signature", header, payload));

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_missing_exp_claim() {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(r#"{"sub":"user123"}"#);
    let token = CloudAuthToken::new(format!("{}.{}.signature", header, payload));

    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_empty_token() {
    let token = CloudAuthToken::new("");
    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_decode_expiry_empty_parts() {
    let token = CloudAuthToken::new("..");
    let result = decode_expiry(&token);
    assert!(result.is_err());
}

#[test]
fn test_is_expired_with_expired_token() {
    let token = create_token_with_offset(-3600);
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_with_valid_token() {
    let token = create_token_with_offset(3600);
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_just_expired() {
    let token = create_token_with_offset(-1);
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_about_to_expire() {
    let token = create_token_with_offset(1);
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_with_invalid_token() {
    let token = CloudAuthToken::new("invalid.token.here");
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_with_malformed_token() {
    let token = CloudAuthToken::new("not_a_jwt");
    assert!(is_expired(&token));
}

#[test]
fn test_is_expired_far_future() {
    let token = create_token_with_offset(86400 * 365);
    assert!(!is_expired(&token));
}

#[test]
fn test_is_expired_far_past() {
    let token = create_token_with_offset(-86400 * 365);
    assert!(is_expired(&token));
}

#[test]
fn test_expires_within_token_expiring_soon() {
    let token = create_token_with_offset(1800);
    assert!(expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_token_not_expiring_soon() {
    let token = create_token_with_offset(7200);
    assert!(!expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_already_expired() {
    let token = create_token_with_offset(-3600);
    assert!(expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_exactly_at_threshold() {
    let token = create_token_with_offset(3600);
    assert!(!expires_within(&token, Duration::hours(1)));
}

#[test]
fn test_expires_within_zero_duration() {
    let token = create_token_with_offset(60);
    assert!(!expires_within(&token, Duration::zero()));
}

#[test]
fn test_expires_within_negative_duration() {
    let token = create_token_with_offset(3600);
    assert!(!expires_within(&token, Duration::hours(-1)));
}

#[test]
fn test_expires_within_with_invalid_token() {
    let token = CloudAuthToken::new("invalid.token");
    assert!(expires_within(&token, Duration::hours(24)));
}

#[test]
fn test_expires_within_various_durations() {
    let token = create_token_with_offset(1800);

    assert!(expires_within(&token, Duration::hours(1)));
    assert!(expires_within(&token, Duration::minutes(45)));
    assert!(!expires_within(&token, Duration::minutes(15)));
    assert!(!expires_within(&token, Duration::minutes(10)));
}

#[test]
fn test_expires_within_days() {
    let token = create_token_with_offset(86400 * 5);

    assert!(expires_within(&token, Duration::days(7)));
    assert!(!expires_within(&token, Duration::days(3)));
}
