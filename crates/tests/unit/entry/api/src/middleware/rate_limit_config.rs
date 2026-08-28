//! Unit tests for `RateLimitConfig` presets.

use systemprompt_models::config::RateLimitConfig;

#[test]
fn default_config_is_not_disabled() {
    let config = RateLimitConfig::default();
    assert!(!config.disabled);
}

#[test]
fn production_config_matches_default() {
    let prod = RateLimitConfig::production();
    let default = RateLimitConfig::default();
    assert_eq!(
        prod.oauth_public_per_second,
        default.oauth_public_per_second
    );
    assert_eq!(prod.burst_multiplier, default.burst_multiplier);
}

#[test]
fn testing_config_has_high_limits() {
    let config = RateLimitConfig::testing();
    assert_eq!(config.oauth_public_per_second, 10000);
    assert_eq!(config.contexts_per_second, 10000);
}

#[test]
fn disabled_config_is_disabled() {
    let config = RateLimitConfig::disabled();
    assert!(config.disabled);
}
