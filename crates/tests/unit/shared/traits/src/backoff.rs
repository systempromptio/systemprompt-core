use std::collections::HashSet;
use std::time::Duration;

use systemprompt_traits::backoff::jittered;

#[test]
fn a_jittered_delay_stays_in_the_upper_half() {
    let base = Duration::from_millis(1_000);
    for _ in 0..500 {
        let d = jittered(base);
        assert!(d >= Duration::from_millis(500) && d <= base, "{d:?}");
    }
}

#[test]
fn jittered_delays_differ_across_draws() {
    let draws: HashSet<Duration> = (0..200)
        .map(|_| jittered(Duration::from_secs(60)))
        .collect();
    assert!(draws.len() > 100, "only {} distinct delays", draws.len());
}

#[test]
fn a_delay_too_short_to_split_is_returned_unchanged() {
    assert_eq!(jittered(Duration::from_millis(1)), Duration::from_millis(1));
    assert_eq!(jittered(Duration::ZERO), Duration::ZERO);
}
