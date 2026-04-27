//! Unit tests for RateLimitConfig and TieredRateLimiter
//!
//! Tests cover:
//! - Default config values
//! - Production and testing presets
//! - Disabled config
//! - Tier multiplier lookups
//! - effective_limit calculations
//! - TieredRateLimiter construction and check behavior

use systemprompt_api::services::middleware::TieredRateLimiter;
use systemprompt_models::auth::RateLimitTier;
use systemprompt_models::config::RateLimitConfig;

#[test]
fn default_config_is_not_disabled() {
    let config = RateLimitConfig::default();
    assert!(!config.disabled);
}

#[test]
fn default_burst_multiplier() {
    let config = RateLimitConfig::default();
    assert_eq!(config.burst_multiplier, 3);
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

#[test]
fn admin_tier_multiplier_is_highest() {
    let config = RateLimitConfig::default();
    let admin = config.tier_multiplier(RateLimitTier::Admin);
    let user = config.tier_multiplier(RateLimitTier::User);
    assert!(admin > user);
}

#[test]
fn anon_tier_multiplier_is_lowest() {
    let config = RateLimitConfig::default();
    let anon = config.tier_multiplier(RateLimitTier::Anon);
    let user = config.tier_multiplier(RateLimitTier::User);
    assert!(anon < user);
}

#[test]
fn user_tier_multiplier_is_one() {
    let config = RateLimitConfig::default();
    let user = config.tier_multiplier(RateLimitTier::User);
    assert!((user - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a2a_and_mcp_tiers_have_same_multiplier() {
    let config = RateLimitConfig::default();
    let a2a = config.tier_multiplier(RateLimitTier::A2a);
    let mcp = config.tier_multiplier(RateLimitTier::Mcp);
    assert!((a2a - mcp).abs() < f64::EPSILON);
}

#[test]
fn effective_limit_scales_by_admin_multiplier() {
    let config = RateLimitConfig::default();
    let base = 100;
    let effective = config.effective_limit(base, RateLimitTier::Admin);
    assert!(effective > base);
}

#[test]
fn effective_limit_scales_by_anon_multiplier() {
    let config = RateLimitConfig::default();
    let base = 100;
    let effective = config.effective_limit(base, RateLimitTier::Anon);
    assert!(effective < base);
}

#[test]
fn effective_limit_user_matches_base() {
    let config = RateLimitConfig::default();
    let base = 100;
    let effective = config.effective_limit(base, RateLimitTier::User);
    assert_eq!(effective, base);
}

#[test]
fn effective_limit_never_zero() {
    let config = RateLimitConfig::default();
    let effective = config.effective_limit(0, RateLimitTier::Anon);
    assert!(effective >= 1);
}

#[test]
fn tiered_limiter_disabled_allows_all() {
    let limiter = TieredRateLimiter::disabled();
    assert!(limiter.check(RateLimitTier::Anon, "test-key"));
    assert!(limiter.check(RateLimitTier::Admin, "test-key"));
}

#[test]
fn tiered_limiter_from_default_config() {
    let config = RateLimitConfig::default();
    let limiter = TieredRateLimiter::new(&config, 100);
    assert!(limiter.check(RateLimitTier::User, "fresh-key"));
}

#[test]
fn tiered_limiter_admin_tier_allows_request() {
    let config = RateLimitConfig::default();
    let limiter = TieredRateLimiter::new(&config, 100);
    assert!(limiter.check(RateLimitTier::Admin, "admin-key"));
}

#[test]
fn tiered_limiter_anon_tier_allows_first_request() {
    let config = RateLimitConfig::default();
    let limiter = TieredRateLimiter::new(&config, 100);
    assert!(limiter.check(RateLimitTier::Anon, "anon-key"));
}

#[test]
fn tiered_limiter_disabled_config_allows_all() {
    let config = RateLimitConfig::disabled();
    let limiter = TieredRateLimiter::new(&config, 10);
    assert!(limiter.check(RateLimitTier::Anon, "key"));
}
