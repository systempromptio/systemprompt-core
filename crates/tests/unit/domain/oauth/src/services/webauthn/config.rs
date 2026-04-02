//! Tests for WebAuthnConfig builder methods and conversions

use std::time::Duration;
use systemprompt_oauth::services::WebAuthnConfig;
use url::Url;

const TEST_RP_ID: &str = "example.com";
const TEST_RP_NAME: &str = "Test RP";
const TEST_ORIGIN: &str = "https://example.com";

fn create_test_config() -> WebAuthnConfig {
    WebAuthnConfig {
        rp_id: TEST_RP_ID.to_string(),
        rp_origin: Url::parse(TEST_ORIGIN).unwrap(),
        rp_name: TEST_RP_NAME.to_string(),
        challenge_expiry: Duration::from_secs(300),
        allow_any_port: false,
        allow_subdomains: false,
    }
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_webauthn_config_construction() {
    let config = create_test_config();

    assert_eq!(config.rp_id, TEST_RP_ID);
    assert_eq!(config.rp_name, TEST_RP_NAME);
    assert_eq!(config.rp_origin.as_str(), "https://example.com/");
    assert_eq!(config.challenge_expiry, Duration::from_secs(300));
    assert!(!config.allow_any_port);
    assert!(!config.allow_subdomains);
}

// ============================================================================
// Builder Method Tests
// ============================================================================

#[test]
fn test_webauthn_config_with_rp_id() {
    let config = create_test_config().with_rp_id("other.example.com");

    assert_eq!(config.rp_id, "other.example.com");
    assert_eq!(config.rp_name, TEST_RP_NAME);
}

#[test]
fn test_webauthn_config_with_rp_name() {
    let config = create_test_config().with_rp_name("My Custom RP");

    assert_eq!(config.rp_name, "My Custom RP");
    assert_eq!(config.rp_id, TEST_RP_ID);
}

#[test]
fn test_webauthn_config_with_rp_origin() {
    let new_origin = Url::parse("https://other.example.com").unwrap();
    let config = create_test_config().with_rp_origin(new_origin.clone());

    assert_eq!(config.rp_origin, new_origin);
    assert_eq!(config.rp_id, TEST_RP_ID);
}

#[test]
fn test_webauthn_config_with_challenge_expiry() {
    let config = create_test_config().with_challenge_expiry(Duration::from_secs(600));

    assert_eq!(config.challenge_expiry, Duration::from_secs(600));
}

#[test]
fn test_webauthn_config_with_any_port() {
    let config = create_test_config().with_any_port(true);

    assert!(config.allow_any_port);
    assert!(!config.allow_subdomains);
}

#[test]
fn test_webauthn_config_with_subdomains() {
    let config = create_test_config().with_subdomains(true);

    assert!(config.allow_subdomains);
    assert!(!config.allow_any_port);
}

// ============================================================================
// Chrono Duration Conversion Tests
// ============================================================================

#[test]
fn test_webauthn_config_challenge_expiry_chrono() {
    let config = create_test_config();

    let chrono_duration = config.challenge_expiry_chrono();
    assert_eq!(chrono_duration.num_seconds(), 300);
}

#[test]
fn test_webauthn_config_challenge_expiry_chrono_large_value() {
    let config = create_test_config().with_challenge_expiry(Duration::from_secs(86400));

    let chrono_duration = config.challenge_expiry_chrono();
    assert_eq!(chrono_duration.num_seconds(), 86400);
}

// ============================================================================
// Debug Tests
// ============================================================================

#[test]
fn test_webauthn_config_debug() {
    let config = create_test_config();
    let debug_output = format!("{config:?}");

    assert!(debug_output.contains("WebAuthnConfig"));
    assert!(debug_output.contains(TEST_RP_ID));
    assert!(debug_output.contains(TEST_RP_NAME));
}
