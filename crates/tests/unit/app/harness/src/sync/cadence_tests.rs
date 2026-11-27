use std::time::Duration;

use systemprompt_harness::sync::cadence::{
    BACKOFF_CAP, Jitter, UNFOCUSED_INTERVAL, after, doubling, enrol_delay, outbox_delay,
    poll_interval, service_delay, transient_delay,
};

use crate::sync::fakes::Fixed;

pub(crate) struct Sequence(pub(crate) Vec<f64>, pub(crate) usize);

impl Jitter for Sequence {
    fn unit(&mut self) -> f64 {
        let value = self.0[self.1 % self.0.len()];
        self.1 += 1;
        value
    }
}

#[test]
fn focused_polls_land_within_thirty_seconds_plus_or_minus_twenty_percent() {
    assert_eq!(
        poll_interval(true, &mut Fixed(0.0)),
        Duration::from_secs(24)
    );
    assert_eq!(
        poll_interval(true, &mut Fixed(0.5)),
        Duration::from_secs(30)
    );
    assert_eq!(
        poll_interval(true, &mut Fixed(1.0)),
        Duration::from_secs(36)
    );
    assert_eq!(
        poll_interval(true, &mut Fixed(f64::NAN)),
        Duration::from_secs(30)
    );
    let mut varied = Sequence(vec![0.1, 0.9, 0.4], 0);
    let gaps: Vec<Duration> = (0..3).map(|_| poll_interval(true, &mut varied)).collect();
    assert!(
        gaps.iter()
            .all(|gap| *gap >= Duration::from_secs(24) && *gap <= Duration::from_secs(36))
    );
    assert_ne!(gaps[0], gaps[1]);
}

#[test]
fn unfocused_polls_wait_five_minutes_without_jitter() {
    assert_eq!(poll_interval(false, &mut Fixed(0.0)), UNFOCUSED_INTERVAL);
    assert_eq!(
        poll_interval(false, &mut Fixed(1.0)),
        Duration::from_secs(300)
    );
}

#[test]
fn transient_failures_back_off_thirty_seconds_doubling_to_five_minutes() {
    let waits: Vec<u64> = (1..=6)
        .map(|failures| transient_delay(failures, None).as_secs())
        .collect();
    assert_eq!(waits, vec![30, 60, 120, 240, 300, 300]);
    assert_eq!(
        transient_delay(1, Some(Duration::from_secs(90))),
        Duration::from_secs(90)
    );
    assert_eq!(
        transient_delay(1, Some(Duration::from_mins(15))),
        BACKOFF_CAP
    );
}

#[test]
fn service_failures_retry_after_two_seconds_doubling_to_the_interval() {
    let interval = Duration::from_secs(30);
    let waits: Vec<u64> = (1..=6)
        .map(|failures| service_delay(failures, interval).as_secs())
        .collect();
    assert_eq!(waits, vec![2, 4, 8, 16, 30, 30]);
}

#[test]
fn enrolment_and_outbox_backoffs_cap_at_an_hour() {
    assert_eq!(enrol_delay(1), Duration::from_secs(2));
    assert_eq!(enrol_delay(2), Duration::from_secs(4));
    assert_eq!(enrol_delay(40), Duration::from_secs(3600));
    assert_eq!(outbox_delay(1), Duration::from_secs(2));
    assert_eq!(outbox_delay(5), Duration::from_secs(32));
    assert_eq!(outbox_delay(30), Duration::from_secs(3600));
    assert_eq!(
        doubling(Duration::from_secs(1), 0, Duration::from_secs(10)),
        Duration::from_secs(1)
    );
}

#[test]
fn after_saturates_instead_of_overflowing() {
    let now = chrono::DateTime::<chrono::Utc>::MAX_UTC;
    assert_eq!(after(now, Duration::from_secs(10)), now);
}
