//! Unit tests for SecurityHeadersConfig
//!
//! Tests cover:
//! - Default values for all security header fields
//! - Custom CSP configuration
//! - Enabled flag default

use systemprompt_models::profile::SecurityHeadersConfig;

#[test]
fn default_enabled_is_true() {
    let config = SecurityHeadersConfig::default();
    assert!(config.enabled);
}

#[test]
fn default_hsts_includes_max_age() {
    let config = SecurityHeadersConfig::default();
    assert!(config.hsts.contains("max-age="));
}

#[test]
fn default_hsts_includes_subdomains() {
    let config = SecurityHeadersConfig::default();
    assert!(config.hsts.contains("includeSubDomains"));
}

#[test]
fn default_hsts_includes_preload() {
    let config = SecurityHeadersConfig::default();
    assert!(config.hsts.contains("preload"));
}

#[test]
fn default_frame_options_is_deny() {
    let config = SecurityHeadersConfig::default();
    assert_eq!(
        config.frame_options,
        systemprompt_extension::FrameOptions::Deny
    );
}

#[test]
fn default_content_type_options_is_nosniff() {
    let config = SecurityHeadersConfig::default();
    assert_eq!(config.content_type_options, "nosniff");
}

#[test]
fn default_referrer_policy() {
    let config = SecurityHeadersConfig::default();
    assert_eq!(
        config.referrer_policy.header_value(),
        "strict-origin-when-cross-origin"
    );
}

#[test]
fn default_permissions_policy_denies_camera() {
    let config = SecurityHeadersConfig::default();
    assert!(config.permissions_policy.contains("camera=()"));
}

#[test]
fn default_permissions_policy_denies_microphone() {
    let config = SecurityHeadersConfig::default();
    assert!(config.permissions_policy.contains("microphone=()"));
}

#[test]
fn default_permissions_policy_denies_geolocation() {
    let config = SecurityHeadersConfig::default();
    assert!(config.permissions_policy.contains("geolocation=()"));
}

#[test]
fn default_csp_is_none() {
    let config = SecurityHeadersConfig::default();
    assert!(config.content_security_policy.is_none());
}

#[test]
fn custom_csp_can_be_set() {
    let mut config = SecurityHeadersConfig::default();
    config.content_security_policy = Some("default-src 'self'".to_string());
    assert_eq!(
        config.content_security_policy.as_deref(),
        Some("default-src 'self'")
    );
}

#[test]
fn all_default_values_are_valid_http_header_values() {
    // Only the free-text fields can carry a value that is not a legal header;
    // frame_options and referrer_policy render from closed enums, so asserting
    // on them would test the type system rather than the defaults.
    let config = SecurityHeadersConfig::default();
    assert!(config.hsts.parse::<http::HeaderValue>().is_ok());
    assert!(
        config
            .content_type_options
            .parse::<http::HeaderValue>()
            .is_ok()
    );
    assert!(
        config
            .permissions_policy
            .parse::<http::HeaderValue>()
            .is_ok()
    );
}
