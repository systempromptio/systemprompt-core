//! Tests for `CircuitBreaker`.

use std::time::Duration;

use systemprompt_database::resilience::breaker::CircuitBreaker;
use systemprompt_database::resilience::config::BreakerConfig;

fn config() -> BreakerConfig {
    BreakerConfig {
        failure_threshold: 3,
        open_cooldown: Duration::from_millis(20),
        half_open_max_probes: 1,
    }
}

#[test]
fn opens_after_consecutive_failures_reach_threshold() {
    let breaker = CircuitBreaker::new("dep", config());

    assert!(breaker.acquire().is_ok());
    breaker.record_failure();
    breaker.record_failure();
    assert!(!breaker.is_open());

    breaker.record_failure();
    assert!(breaker.is_open());
    assert!(breaker.acquire().is_err());
}

#[test]
fn success_resets_the_failure_count() {
    let breaker = CircuitBreaker::new("dep", config());

    breaker.record_failure();
    breaker.record_failure();
    breaker.record_success();
    breaker.record_failure();
    breaker.record_failure();

    assert!(!breaker.is_open());
}

#[tokio::test]
async fn half_open_probe_recovers_the_breaker() {
    let breaker = CircuitBreaker::new("dep", config());
    for _ in 0..3 {
        breaker.record_failure();
    }
    assert!(breaker.acquire().is_err());

    tokio::time::sleep(Duration::from_millis(35)).await;

    // Cooldown elapsed: the first acquire is admitted as a half-open probe.
    assert!(breaker.acquire().is_ok());
    // A concurrent second probe is rejected (half_open_max_probes = 1).
    assert!(breaker.acquire().is_err());

    breaker.record_success();
    assert!(!breaker.is_open());
    assert!(breaker.acquire().is_ok());
}

#[tokio::test]
async fn half_open_probe_failure_reopens_the_breaker() {
    let breaker = CircuitBreaker::new("dep", config());
    for _ in 0..3 {
        breaker.record_failure();
    }

    tokio::time::sleep(Duration::from_millis(35)).await;
    assert!(breaker.acquire().is_ok());

    breaker.record_failure();
    assert!(breaker.is_open());
}
