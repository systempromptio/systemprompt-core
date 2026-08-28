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
