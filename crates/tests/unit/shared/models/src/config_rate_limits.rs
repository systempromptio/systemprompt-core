use systemprompt_models::auth::RateLimitTier;
use systemprompt_models::config::RateLimitConfig;

#[test]
fn rate_limit_config_default_has_sensible_values() {
    let cfg = RateLimitConfig::default();
    assert!(!cfg.disabled);
    assert!(cfg.burst_multiplier > 0);
    assert!(cfg.contexts_per_second > 0);
    assert!(cfg.mcp_per_second > 0);
}

#[test]
fn rate_limit_config_production_equals_default() {
    let prod = RateLimitConfig::production();
    let def = RateLimitConfig::default();
    assert_eq!(prod.contexts_per_second, def.contexts_per_second);
    assert_eq!(prod.disabled, def.disabled);
}

#[test]
fn rate_limit_config_testing_has_high_limits() {
    let cfg = RateLimitConfig::testing();
    assert!(cfg.contexts_per_second >= 10000);
    assert!(cfg.mcp_per_second >= 10000);
    assert!(!cfg.disabled);
}

#[test]
fn rate_limit_config_disabled_sets_disabled_flag() {
    let cfg = RateLimitConfig::disabled();
    assert!(cfg.disabled);
    assert!(cfg.contexts_per_second >= 10000);
}

#[test]
fn rate_limit_config_effective_limit_with_admin_tier() {
    let cfg = RateLimitConfig::default();
    let limit = cfg.effective_limit(100, RateLimitTier::Admin);
    let multiplier = cfg.tier_multiplier(RateLimitTier::Admin);
    let expected = (100.0 * multiplier) as u64;
    assert_eq!(limit, expected);
}

#[test]
fn rate_limit_config_effective_limit_with_anon_tier() {
    let cfg = RateLimitConfig::default();
    let limit = cfg.effective_limit(100, RateLimitTier::Anon);
    assert!(limit >= 1);
}

#[test]
fn rate_limit_config_tier_multiplier_all_tiers() {
    let cfg = RateLimitConfig::default();
    for tier in [
        RateLimitTier::Admin,
        RateLimitTier::User,
        RateLimitTier::A2a,
        RateLimitTier::Mcp,
        RateLimitTier::Service,
        RateLimitTier::Anon,
    ] {
        let m = cfg.tier_multiplier(tier);
        assert!(m > 0.0, "tier {tier:?} multiplier should be positive");
    }
}

#[test]
fn rate_limit_config_effective_limit_clamps_at_one() {
    let cfg = RateLimitConfig::default();
    let limit = cfg.effective_limit(0, RateLimitTier::Anon);
    assert!(limit >= 1);
}
