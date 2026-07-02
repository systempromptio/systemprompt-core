//! Tests for the startup health-poll backoff schedule.

use std::time::Duration;
use systemprompt_mcp::services::lifecycle::startup::calculate_delay;

#[test]
fn first_attempt_uses_fixed_grace_delay() {
    assert_eq!(
        calculate_delay(1, Duration::from_millis(300)),
        Duration::from_millis(500)
    );
}

#[test]
fn subsequent_attempts_scale_linearly() {
    let base = Duration::from_millis(300);
    assert_eq!(calculate_delay(2, base), base * 2);
    assert_eq!(calculate_delay(3, base), base * 3);
    assert_eq!(calculate_delay(5, base), base * 5);
}

#[test]
fn delay_is_capped_at_five_times_base() {
    let base = Duration::from_millis(300);
    assert_eq!(calculate_delay(6, base), base * 5);
    assert_eq!(calculate_delay(15, base), base * 5);
}
