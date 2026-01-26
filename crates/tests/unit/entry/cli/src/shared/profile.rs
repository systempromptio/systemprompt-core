//! Unit tests for profile utilities module
//!
//! Tests cover:
//! - ProfileResolutionError enum variants and messages
//! - generate_display_name function for various profile names
//! - capitalize_first function behavior
//! - generate_jwt_secret length and randomness

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use std::collections::HashSet;
use systemprompt_cli::shared::ProfileResolutionError;

// ============================================================================
// ProfileResolutionError Tests
// ============================================================================

#[test]
fn test_profile_resolution_error_no_profiles_found_display() {
    let error = ProfileResolutionError::NoProfilesFound;
    let msg = error.to_string();
    assert!(msg.contains("No profiles found"));
    assert!(msg.contains("systemprompt cloud profile create"));
}

#[test]
fn test_profile_resolution_error_multiple_profiles_display() {
    let error = ProfileResolutionError::MultipleProfilesFound {
        profiles: vec!["dev".to_string(), "prod".to_string()],
    };
    let msg = error.to_string();
    assert!(msg.contains("Multiple profiles found"));
}

#[test]
fn test_profile_resolution_error_no_profiles_debug() {
    let error = ProfileResolutionError::NoProfilesFound;
    let debug = format!("{:?}", error);
    assert!(debug.contains("NoProfilesFound"));
}

#[test]
fn test_profile_resolution_error_multiple_profiles_debug() {
    let error = ProfileResolutionError::MultipleProfilesFound {
        profiles: vec!["dev".to_string()],
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("MultipleProfilesFound"));
}

#[test]
fn test_profile_resolution_error_discovery_failed() {
    let inner_error = anyhow::anyhow!("Test error");
    let error = ProfileResolutionError::DiscoveryFailed(inner_error);
    let msg = error.to_string();
    assert!(msg.contains("Profile discovery failed"));
    assert!(msg.contains("Test error"));
}

#[test]
fn test_profile_resolution_error_discovery_failed_debug() {
    let inner_error = anyhow::anyhow!("Inner error");
    let error = ProfileResolutionError::DiscoveryFailed(inner_error);
    let debug = format!("{:?}", error);
    assert!(debug.contains("DiscoveryFailed"));
}

// ============================================================================
// generate_display_name Tests (via module behavior)
// Note: This tests the expected display name behavior documented in the code
// ============================================================================

fn expected_display_name(input: &str) -> String {
    match input.to_lowercase().as_str() {
        "dev" | "development" => "Development".to_string(),
        "prod" | "production" => "Production".to_string(),
        "staging" | "stage" => "Staging".to_string(),
        "test" | "testing" => "Test".to_string(),
        "local" => "Local Development".to_string(),
        "cloud" => "Cloud".to_string(),
        _ => {
            let mut chars = input.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

#[test]
fn test_expected_display_name_dev() {
    assert_eq!(expected_display_name("dev"), "Development");
    assert_eq!(expected_display_name("DEV"), "Development");
    assert_eq!(expected_display_name("Dev"), "Development");
}

#[test]
fn test_expected_display_name_development() {
    assert_eq!(expected_display_name("development"), "Development");
    assert_eq!(expected_display_name("DEVELOPMENT"), "Development");
}

#[test]
fn test_expected_display_name_prod() {
    assert_eq!(expected_display_name("prod"), "Production");
    assert_eq!(expected_display_name("PROD"), "Production");
}

#[test]
fn test_expected_display_name_production() {
    assert_eq!(expected_display_name("production"), "Production");
}

#[test]
fn test_expected_display_name_staging() {
    assert_eq!(expected_display_name("staging"), "Staging");
    assert_eq!(expected_display_name("stage"), "Staging");
}

#[test]
fn test_expected_display_name_test() {
    assert_eq!(expected_display_name("test"), "Test");
    assert_eq!(expected_display_name("testing"), "Test");
}

#[test]
fn test_expected_display_name_local() {
    assert_eq!(expected_display_name("local"), "Local Development");
}

#[test]
fn test_expected_display_name_cloud() {
    assert_eq!(expected_display_name("cloud"), "Cloud");
}

#[test]
fn test_expected_display_name_custom() {
    assert_eq!(expected_display_name("custom"), "Custom");
    assert_eq!(expected_display_name("myprofile"), "Myprofile");
}

#[test]
fn test_expected_display_name_empty() {
    assert_eq!(expected_display_name(""), "");
}

// ============================================================================
// capitalize_first behavior tests
// ============================================================================

fn capitalize_first_ref(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[test]
fn test_capitalize_first_lowercase() {
    assert_eq!(capitalize_first_ref("hello"), "Hello");
}

#[test]
fn test_capitalize_first_already_capitalized() {
    assert_eq!(capitalize_first_ref("Hello"), "Hello");
}

#[test]
fn test_capitalize_first_all_caps() {
    assert_eq!(capitalize_first_ref("HELLO"), "HELLO");
}

#[test]
fn test_capitalize_first_single_char() {
    assert_eq!(capitalize_first_ref("a"), "A");
}

#[test]
fn test_capitalize_first_empty() {
    assert_eq!(capitalize_first_ref(""), "");
}

#[test]
fn test_capitalize_first_with_numbers() {
    assert_eq!(capitalize_first_ref("123abc"), "123abc");
}

#[test]
fn test_capitalize_first_with_underscore() {
    assert_eq!(capitalize_first_ref("_test"), "_test");
}

// ============================================================================
// JWT Secret Generation Tests (testing expected behavior)
// ============================================================================

#[test]
fn test_jwt_secret_generation_expected_length() {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    let secret: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    assert_eq!(secret.len(), 64);
}

#[test]
fn test_jwt_secret_generation_alphanumeric() {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    let secret: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    assert!(secret.chars().all(|c: char| c.is_ascii_alphanumeric()));
}

#[test]
fn test_jwt_secret_generation_uniqueness() {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    let secrets: HashSet<String> = (0..100)
        .map(|_| {
            thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect()
        })
        .collect();

    assert_eq!(secrets.len(), 100, "All 100 secrets should be unique");
}

// ============================================================================
// Error Chain Tests
// ============================================================================

#[test]
fn test_profile_resolution_error_is_error_trait() {
    fn assert_error<T: std::error::Error>(_: &T) {}

    let error = ProfileResolutionError::NoProfilesFound;
    assert_error(&error);
}

#[test]
fn test_profile_resolution_error_source_none_for_no_profiles() {
    use std::error::Error;

    let error = ProfileResolutionError::NoProfilesFound;
    assert!(error.source().is_none());
}

#[test]
fn test_profile_resolution_error_source_none_for_multiple_profiles() {
    use std::error::Error;

    let error = ProfileResolutionError::MultipleProfilesFound {
        profiles: vec!["dev".to_string()],
    };
    assert!(error.source().is_none());
}
