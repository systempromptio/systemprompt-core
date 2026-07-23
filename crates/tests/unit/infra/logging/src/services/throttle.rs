//! Unit tests for `LogThrottle` interval-based emission gating.

use systemprompt_logging::LogThrottle;

#[test]
fn first_call_is_allowed() {
    let throttle = LogThrottle::new(3600);
    assert!(throttle.allow_at(1_000));
}

#[test]
fn second_call_within_interval_is_suppressed() {
    let throttle = LogThrottle::new(3600);
    assert!(throttle.allow_at(1_000));
    assert!(!throttle.allow_at(1_000));
    assert!(!throttle.allow_at(4_599));
}

#[test]
fn call_after_interval_is_allowed_again() {
    let throttle = LogThrottle::new(3600);
    assert!(throttle.allow_at(1_000));
    assert!(throttle.allow_at(4_600));
    assert!(!throttle.allow_at(4_601));
}

#[test]
fn allow_uses_wall_clock() {
    let throttle = LogThrottle::new(3600);
    assert!(throttle.allow());
    assert!(!throttle.allow());
}
