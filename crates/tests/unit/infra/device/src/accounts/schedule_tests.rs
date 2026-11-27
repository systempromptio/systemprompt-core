use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_device::accounts::schedule::{RETRY_EVERY, renewal_delay, retry_delay, satisfies};
use systemprompt_device::client::Freshness;

const MIN_WAIT: Duration = Duration::from_secs(10);
const MAX_WAIT: Duration = Duration::from_hours(12);

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_900_000_000 + seconds, 0).unwrap()
}

#[test]
fn renewal_is_due_at_eighty_percent_of_the_lifetime() {
    assert_eq!(
        renewal_delay(at(0), at(3600), at(0)),
        Duration::from_mins(48)
    );
    assert_eq!(
        renewal_delay(at(0), at(3600), at(2000)),
        Duration::from_secs(880)
    );
}

#[test]
fn renewal_waits_are_bounded() {
    assert_eq!(renewal_delay(at(0), at(3600), at(3000)), MIN_WAIT);
    assert_eq!(renewal_delay(at(0), at(0), at(0)), MIN_WAIT);
    assert_eq!(renewal_delay(at(10), at(0), at(0)), MIN_WAIT);
    assert_eq!(renewal_delay(at(0), at(400 * 24 * 3600), at(0)), MAX_WAIT);
}

#[test]
fn retries_run_every_thirty_seconds_while_the_token_holds() {
    assert_eq!(RETRY_EVERY, Duration::from_secs(30));
    assert_eq!(retry_delay(at(3600), at(0)), Some(RETRY_EVERY));
    assert_eq!(
        retry_delay(at(3600), at(3590)),
        Some(Duration::from_secs(10))
    );
    assert_eq!(retry_delay(at(3600), at(3600)), None);
    assert_eq!(retry_delay(at(3600), at(4000)), None);
}

#[test]
fn freshness_is_checked_against_the_expiry() {
    assert!(satisfies(at(100), at(0), Freshness::Any));
    assert!(!satisfies(at(0), at(0), Freshness::Any));
    assert!(satisfies(
        at(100),
        at(0),
        Freshness::ValidFor(Duration::from_secs(60))
    ));
    assert!(!satisfies(
        at(100),
        at(0),
        Freshness::ValidFor(Duration::from_secs(100))
    ));
    assert!(!satisfies(at(100_000), at(0), Freshness::Fresh));
}
