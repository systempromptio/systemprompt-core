use std::time::{Duration, Instant};

use sp_protocol::view::SyncErrorKind;

use sp_sync::planner::{Next, Outcome, Planner, Request, Trigger};

use crate::fakes::Fixed;

fn failed(kind: SyncErrorKind) -> Outcome {
    Outcome::Failed {
        kind,
        retry_after: None,
    }
}

#[test]
fn the_first_poll_runs_at_once_and_reads_the_catalog() {
    let mut planner = Planner::new();
    let first = planner.take().unwrap();
    assert_eq!(
        first,
        Request {
            no_cache: false,
            reread: true
        }
    );
    assert!(planner.take().is_none());
}

#[test]
fn requests_made_while_a_poll_runs_merge_into_one() {
    let mut planner = Planner::new();
    planner.take();
    planner.request(Trigger::Timer);
    planner.request(Trigger::Focus);
    planner.request(Trigger::Timer);
    assert_eq!(
        planner.take(),
        Some(Request {
            no_cache: false,
            reread: true
        })
    );
    assert_eq!(planner.take(), None);
}

#[test]
fn only_sync_now_asks_for_a_fresh_build() {
    let mut planner = Planner::new();
    planner.take();
    planner.request(Trigger::Focus);
    assert!(!planner.take().unwrap().no_cache);
    planner.request(Trigger::Timer);
    assert!(!planner.take().unwrap().no_cache);
    planner.request(Trigger::SyncNow);
    assert_eq!(
        planner.take(),
        Some(Request {
            no_cache: true,
            reread: true
        })
    );
}

#[test]
fn the_catalog_is_read_every_five_minutes_unless_asked_for() {
    let mut planner = Planner::new();
    let start = Instant::now();
    let timer = Request::default();
    assert!(planner.catalog_due(timer, start));
    planner.catalog_read(start);
    assert!(!planner.catalog_due(timer, start + Duration::from_secs(30)));
    assert!(planner.catalog_due(
        Request {
            no_cache: false,
            reread: true
        },
        start + Duration::from_secs(30)
    ));
    assert!(planner.catalog_due(timer, start + Duration::from_secs(300)));
}

#[test]
fn a_synced_poll_waits_one_interval() {
    let mut planner = Planner::new();
    assert_eq!(
        planner.finished(Outcome::Synced, true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(30))
    );
    assert_eq!(
        planner.finished(Outcome::Synced, false, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(300))
    );
}

#[test]
fn unreachable_backs_off_and_recovers() {
    let mut planner = Planner::new();
    let waits: Vec<Next> = (0..6)
        .map(|_| planner.finished(failed(SyncErrorKind::Unreachable), true, &mut Fixed(0.5)))
        .collect();
    let expected: Vec<Next> = [30, 60, 120, 240, 300, 300]
        .iter()
        .map(|s| Next::Wait(Duration::from_secs(*s)))
        .collect();
    assert_eq!(waits, expected);
    assert_eq!(
        planner.finished(Outcome::Synced, true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(30))
    );
    let again = planner.finished(failed(SyncErrorKind::Unreachable), true, &mut Fixed(0.5));
    assert_eq!(again, Next::Wait(Duration::from_secs(30)));
}

#[test]
fn rate_limiting_waits_as_long_as_asked_within_the_cap() {
    let mut planner = Planner::new();
    let limited = Outcome::Failed {
        kind: SyncErrorKind::RateLimited,
        retry_after: Some(Duration::from_secs(200)),
    };
    assert_eq!(
        planner.finished(limited, true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(200))
    );
}

#[test]
fn a_failed_apply_retries_soon_and_other_failures_wait_an_interval() {
    let mut planner = Planner::new();
    assert_eq!(
        planner.finished(failed(SyncErrorKind::Service), true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(2))
    );
    assert_eq!(
        planner.finished(failed(SyncErrorKind::Service), true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(4))
    );
    let mut fresh = Planner::new();
    assert_eq!(
        fresh.finished(failed(SyncErrorKind::Invalid), true, &mut Fixed(0.5)),
        Next::Wait(Duration::from_secs(30))
    );
    assert_eq!(
        fresh.finished(Outcome::Halted, true, &mut Fixed(0.5)),
        Next::Halt
    );
}
