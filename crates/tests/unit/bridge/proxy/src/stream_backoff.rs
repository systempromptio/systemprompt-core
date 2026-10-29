use std::time::Duration;

use systemprompt_bridge::proxy::comms::{
    HEALTHY_AFTER, JITTER, RETRY_MAX, RETRY_MIN, ReconnectBackoff, jittered,
};

const BRIEF: Duration = Duration::from_millis(50);

#[test]
fn jitter_stays_within_twenty_percent() {
    let base = Duration::from_secs(10);
    assert!((JITTER - 0.2).abs() < f64::EPSILON);
    assert_eq!(jittered(base, 0.0), Duration::from_secs(8));
    assert_eq!(jittered(base, 1.0), Duration::from_secs(12));
    assert_eq!(jittered(base, 0.5), base);
    for step in 0..=20 {
        let delay = jittered(base, f64::from(step) / 20.0);
        assert!(delay >= Duration::from_secs(8) && delay <= Duration::from_secs(12));
    }
}

#[test]
fn a_clean_close_after_a_brief_connection_keeps_growing_the_delay() {
    let mut backoff = ReconnectBackoff::default();
    let delays: Vec<Duration> = (0..4).map(|_| backoff.next_delay(BRIEF, 0.5)).collect();
    assert_eq!(
        delays,
        vec![RETRY_MIN, RETRY_MIN * 2, RETRY_MIN * 4, RETRY_MIN * 8]
    );
}

#[test]
fn the_delay_resets_only_after_a_healthy_connection() {
    let mut backoff = ReconnectBackoff::default();
    for _ in 0..3 {
        let _ = backoff.next_delay(BRIEF, 0.5);
    }
    assert_eq!(backoff.next_delay(HEALTHY_AFTER, 0.5), RETRY_MIN);
}

#[test]
fn the_delay_is_capped() {
    let mut backoff = ReconnectBackoff::default();
    for _ in 0..20 {
        let _ = backoff.next_delay(BRIEF, 0.5);
    }
    assert_eq!(backoff.base(), RETRY_MAX);
    assert!(backoff.next_delay(BRIEF, 1.0) <= RETRY_MAX.mul_f64(1.0 + JITTER));
}
