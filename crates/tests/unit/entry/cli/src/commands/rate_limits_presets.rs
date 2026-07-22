//! Tests for the rate-limit preset catalogue.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::rate_limits::preset::{
    get_preset_config, get_preset_description,
};
use systemprompt_models::profile::RateLimitsConfig;

#[test]
fn development_preset_is_relaxed() {
    let cfg = get_preset_config("development").unwrap();
    assert!(!cfg.disabled);
    assert_eq!(cfg.oauth_public_per_second, 50);
    assert_eq!(cfg.content_per_second, 200);
    assert_eq!(cfg.burst_multiplier, 5);
    assert_eq!(cfg.tier_multipliers.admin, 10.0);
    assert_eq!(cfg.tier_multipliers.anon, 0.5);
}

#[test]
fn production_preset_matches_defaults() {
    let cfg = get_preset_config("production").unwrap();
    let default = RateLimitsConfig::default();
    assert_eq!(cfg.oauth_public_per_second, default.oauth_public_per_second);
    assert_eq!(cfg.burst_multiplier, default.burst_multiplier);
    assert_eq!(cfg.disabled, default.disabled);
}

#[test]
fn high_traffic_preset_is_strict() {
    let cfg = get_preset_config("high-traffic").unwrap();
    assert_eq!(cfg.oauth_public_per_second, 5);
    assert_eq!(cfg.oauth_auth_per_second, 2);
    assert_eq!(cfg.burst_multiplier, 2);
    assert_eq!(cfg.tier_multipliers.anon, 0.2);
}

#[test]
fn unknown_preset_is_rejected_with_valid_options() {
    let err = get_preset_config("nope").unwrap_err();
    assert!(err.to_string().contains("Unknown preset: nope"));
    assert!(
        err.to_string()
            .contains("development, production, high-traffic")
    );

    let err = get_preset_description("nope").unwrap_err();
    assert!(err.to_string().contains("Unknown preset: nope"));
}

#[test]
fn every_preset_has_a_description() {
    for name in ["development", "production", "high-traffic"] {
        let desc = get_preset_description(name).unwrap();
        assert!(!desc.is_empty());
    }
}
