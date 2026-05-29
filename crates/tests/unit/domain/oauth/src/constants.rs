//! Tests for `systemprompt_oauth::constants`.
//!
//! Every public constant module has at least one test asserting the stored
//! value so accidental config-drift fails loudly in CI.

use systemprompt_oauth::constants::{pkce, token, validation, webauthn};

#[test]
fn pkce_challenge_min_length_is_43() {
    assert_eq!(pkce::CODE_CHALLENGE_MIN_LENGTH, 43);
}

#[test]
fn pkce_challenge_max_length_is_128() {
    assert_eq!(pkce::CODE_CHALLENGE_MAX_LENGTH, 128);
}

#[test]
fn pkce_min_less_than_max() {
    assert!(pkce::CODE_CHALLENGE_MIN_LENGTH < pkce::CODE_CHALLENGE_MAX_LENGTH);
}

#[test]
fn token_cookie_max_age_is_one_hour() {
    assert_eq!(token::COOKIE_MAX_AGE_SECONDS, 3600);
}

#[test]
fn token_seconds_per_day_is_86400() {
    assert_eq!(token::SECONDS_PER_DAY, 86_400);
}

#[test]
fn token_refresh_token_expiry_days_is_30() {
    assert_eq!(token::REFRESH_TOKEN_EXPIRY_DAYS, 30);
}

#[test]
fn token_anonymous_expiry_equals_one_day() {
    assert_eq!(
        token::ANONYMOUS_TOKEN_EXPIRY_SECONDS,
        token::SECONDS_PER_DAY
    );
}

#[test]
fn webauthn_challenge_expiry_is_300_seconds() {
    assert_eq!(webauthn::CHALLENGE_EXPIRY_SECONDS, 300);
}

#[test]
fn webauthn_cleanup_interval_equals_challenge_expiry() {
    assert_eq!(
        webauthn::CLEANUP_INTERVAL_SECONDS,
        webauthn::CHALLENGE_EXPIRY_SECONDS
    );
}

#[test]
fn validation_min_sequential_run_is_6() {
    assert_eq!(validation::MIN_SEQUENTIAL_RUN, 6);
}

#[test]
fn validation_diversity_threshold_is_half() {
    assert!((validation::DIVERSITY_THRESHOLD - 0.5).abs() < f64::EPSILON);
}

#[test]
fn validation_min_unique_chars_is_20() {
    assert_eq!(validation::MIN_UNIQUE_CHARS, 20);
}
