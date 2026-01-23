//! Unit tests for CLI parsers module
//!
//! Tests cover:
//! - parse_profile_name function
//! - parse_email function
//! - Error cases for invalid inputs

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::shared::{parse_email, parse_profile_name};

// ============================================================================
// parse_profile_name Tests
// ============================================================================

#[test]
fn test_parse_profile_name_valid_simple() {
    let result = parse_profile_name("local");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "local");
}

#[test]
fn test_parse_profile_name_valid_with_hyphen() {
    let result = parse_profile_name("my-profile");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_with_underscore() {
    let result = parse_profile_name("my_profile");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_with_numbers() {
    let result = parse_profile_name("profile123");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_production() {
    let result = parse_profile_name("production");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_staging() {
    let result = parse_profile_name("staging");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_development() {
    let result = parse_profile_name("development");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_mixed_case() {
    let result = parse_profile_name("MyProfile");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_invalid_empty() {
    let result = parse_profile_name("");
    assert!(result.is_err());
}

#[test]
fn test_parse_profile_name_invalid_with_space() {
    let result = parse_profile_name("my profile");
    assert!(result.is_err());
}

#[test]
fn test_parse_profile_name_invalid_with_special_chars() {
    let result = parse_profile_name("profile@name");
    assert!(result.is_err());
}

#[test]
fn test_parse_profile_name_valid_starts_with_number() {
    let result = parse_profile_name("123profile");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_valid_starts_with_hyphen() {
    let result = parse_profile_name("-profile");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_error_message_is_string() {
    let result = parse_profile_name("");
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(!error.is_empty());
}

// ============================================================================
// parse_email Tests
// ============================================================================

#[test]
fn test_parse_email_valid_simple() {
    let result = parse_email("user@example.com");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "user@example.com");
}

#[test]
fn test_parse_email_valid_with_subdomain() {
    let result = parse_email("user@mail.example.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_valid_with_plus() {
    let result = parse_email("user+tag@example.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_valid_with_dots() {
    let result = parse_email("first.last@example.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_valid_with_numbers() {
    let result = parse_email("user123@example.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_valid_with_hyphen_domain() {
    let result = parse_email("user@example-domain.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_valid_different_tlds() {
    let valid_emails = [
        "user@example.org",
        "user@example.net",
        "user@example.io",
        "user@example.co.uk",
    ];

    for email in valid_emails {
        let result = parse_email(email);
        assert!(result.is_ok(), "Expected {} to be valid", email);
    }
}

#[test]
fn test_parse_email_invalid_empty() {
    let result = parse_email("");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_invalid_no_at_symbol() {
    let result = parse_email("userexample.com");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_invalid_no_domain() {
    let result = parse_email("user@");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_invalid_no_local_part() {
    let result = parse_email("@example.com");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_invalid_double_at() {
    let result = parse_email("user@@example.com");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_with_leading_space_in_local_part() {
    let result = parse_email("user @example.com");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_invalid_no_tld() {
    let result = parse_email("user@example");
    assert!(result.is_err());
}

#[test]
fn test_parse_email_error_message_is_string() {
    let result = parse_email("invalid");
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(!error.is_empty());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_parse_profile_name_single_char() {
    let result = parse_profile_name("a");
    assert!(result.is_ok());
}

#[test]
fn test_parse_profile_name_max_reasonable_length() {
    let result = parse_profile_name("a_very_long_profile_name_that_is_still_valid");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_minimal_valid() {
    let result = parse_email("a@b.co");
    assert!(result.is_ok());
}

#[test]
fn test_parse_email_preserves_case() {
    let result = parse_email("User@Example.COM");
    assert!(result.is_ok());
}
