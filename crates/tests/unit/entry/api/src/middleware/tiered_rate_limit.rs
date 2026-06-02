//! Exhaustion and per-tier isolation behaviour for `TieredRateLimiter`.
//!
//! Complements `rate_limit_config.rs` (which only checks first-request
//! admission) by driving the keyed limiter past its burst budget and asserting
//! the deny path, key isolation, and trusted-proxy plumbing.

use ipnet::IpNet;
use systemprompt_api::services::middleware::TieredRateLimiter;
use systemprompt_models::auth::RateLimitTier;
use systemprompt_models::config::RateLimitConfig;

// A config with the smallest possible burst budget so the limiter can be
// driven to denial in a bounded loop.
fn tiny_config() -> RateLimitConfig {
    RateLimitConfig {
        burst_multiplier: 1,
        ..RateLimitConfig::default()
    }
}

#[test]
fn check_eventually_denies_when_burst_exhausted() {
    let cfg = tiny_config();
    // base_per_second = 1 with User multiplier 1.0 and burst_multiplier 1 keeps
    // the bucket tiny.
    let limiter = TieredRateLimiter::new(&cfg, 1);
    let mut denied = false;
    for _ in 0..50 {
        if !limiter.check(RateLimitTier::User, "burst-key") {
            denied = true;
            break;
        }
    }
    assert!(denied, "limiter should deny once burst budget is spent");
}

#[test]
fn distinct_keys_have_independent_buckets() {
    let cfg = tiny_config();
    let limiter = TieredRateLimiter::new(&cfg, 1);

    // Drain key A to denial.
    let mut a_denied = false;
    for _ in 0..50 {
        if !limiter.check(RateLimitTier::User, "key-a") {
            a_denied = true;
            break;
        }
    }
    assert!(a_denied);

    // A fresh key is still admitted.
    assert!(limiter.check(RateLimitTier::User, "key-b"));
}

#[test]
fn disabled_limiter_never_denies_even_under_load() {
    let limiter = TieredRateLimiter::disabled();
    for _ in 0..1000 {
        assert!(limiter.check(RateLimitTier::Anon, "hammer"));
    }
}

#[test]
fn disabled_config_constructed_limiter_never_denies() {
    let limiter = TieredRateLimiter::new(&RateLimitConfig::disabled(), 1);
    for _ in 0..1000 {
        assert!(limiter.check(RateLimitTier::Mcp, "hammer"));
    }
}

#[test]
fn all_tiers_admit_a_fresh_key() {
    let limiter = TieredRateLimiter::new(&RateLimitConfig::testing(), 100);
    for tier in [
        RateLimitTier::Admin,
        RateLimitTier::User,
        RateLimitTier::A2a,
        RateLimitTier::Mcp,
        RateLimitTier::Service,
        RateLimitTier::Anon,
    ] {
        assert!(limiter.check(tier, "fresh"), "tier {tier:?} should admit");
    }
}

#[test]
fn new_limiter_has_no_trusted_proxies() {
    let limiter = TieredRateLimiter::new(&RateLimitConfig::default(), 100);
    assert!(limiter.trusted_proxies().is_empty());
}

#[test]
fn disabled_limiter_has_no_trusted_proxies() {
    let limiter = TieredRateLimiter::disabled();
    assert!(limiter.trusted_proxies().is_empty());
}

#[test]
fn with_trusted_proxies_exposes_supplied_cidrs() {
    let cidrs: Vec<IpNet> = vec![
        "10.0.0.0/8".parse().expect("cidr"),
        "192.168.1.1/32".parse().expect("cidr"),
    ];
    let limiter =
        TieredRateLimiter::with_trusted_proxies(&RateLimitConfig::default(), 100, cidrs.clone());
    assert_eq!(limiter.trusted_proxies(), cidrs.as_slice());
}

#[test]
fn limiter_is_cloneable_and_shares_state() {
    let cfg = tiny_config();
    let limiter = TieredRateLimiter::new(&cfg, 1);
    let clone = limiter.clone();

    // Exhaust through the original.
    for _ in 0..50 {
        if !limiter.check(RateLimitTier::User, "shared") {
            break;
        }
    }
    // The clone shares the Arc-backed bucket, so the same key stays denied.
    assert!(!clone.check(RateLimitTier::User, "shared"));
}
