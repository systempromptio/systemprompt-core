//! `MemoryRateLimitBuckets` keeps one counter per caller and scope in a
//! bounded, sharded least-recently-charged order: a caller's next window
//! restarts its count, a full shard evicts only a counter from an earlier
//! window, and a shard whose every counter is still current refuses the new
//! caller rather than forgetting a spent budget.

use std::num::NonZeroUsize;

use chrono::{DateTime, Duration, Utc};
use systemprompt_api::services::middleware::MemoryRateLimitBuckets;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RateLimitBuckets;

const SHARDS: usize = MemoryRateLimitBuckets::SHARDS;

fn window(offset_secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000 + offset_secs, 0).expect("timestamp")
}

#[tokio::test]
async fn a_caller_is_counted_within_a_window_and_restarts_in_the_next() {
    let buckets =
        MemoryRateLimitBuckets::with_capacity(std::num::NonZeroU64::new(262_144).unwrap());
    let user = UserId::new("memory-shards-counter");
    for expected in 1..=3 {
        assert_eq!(
            buckets.hit(&user, "scope", window(0)).await.unwrap(),
            expected
        );
    }
    assert_eq!(buckets.hit(&user, "scope", window(10)).await.unwrap(), 1);
    assert_eq!(buckets.hit(&user, "other", window(10)).await.unwrap(), 1);
    assert_eq!(buckets.tracked(), 2);
}

#[tokio::test]
async fn a_full_shard_evicts_only_a_counter_from_an_earlier_window() {
    let buckets = MemoryRateLimitBuckets::with_shard_capacity(NonZeroUsize::MIN);
    let callers = SHARDS * 4;
    for caller in 0..callers {
        let user = UserId::new(format!("memory-shards-old-{caller}"));
        let _ = buckets.hit(&user, "scope", window(0)).await;
    }
    assert!(buckets.tracked() <= SHARDS);

    let before = buckets.evictions();
    for caller in 0..callers {
        let user = UserId::new(format!("memory-shards-new-{caller}"));
        let _ = buckets.hit(&user, "scope", window(10)).await;
    }
    assert!(buckets.evictions() > before);
    assert!(buckets.tracked() <= SHARDS);
}

#[tokio::test]
async fn a_shard_of_current_counters_refuses_a_new_caller() {
    let buckets = MemoryRateLimitBuckets::with_shard_capacity(NonZeroUsize::MIN);
    let mut refused = 0;
    for caller in 0..(SHARDS * 8) {
        let user = UserId::new(format!("memory-shards-current-{caller}"));
        if buckets.hit(&user, "scope", window(0)).await.is_err() {
            refused += 1;
        }
    }
    assert!(refused > 0);
    assert_eq!(buckets.refusals(), refused);
    assert_eq!(buckets.evictions(), 0);
}

#[tokio::test]
async fn concurrent_charges_of_one_caller_are_all_counted() {
    let buckets = std::sync::Arc::new(MemoryRateLimitBuckets::with_capacity(
        std::num::NonZeroU64::new(262_144).unwrap(),
    ));
    let user = UserId::new("memory-shards-concurrent");
    let start = Utc::now() - Duration::seconds(1);
    let mut tasks = Vec::new();
    for _ in 0..64 {
        let buckets = std::sync::Arc::clone(&buckets);
        let user = user.clone();
        tasks.push(tokio::spawn(async move {
            buckets.hit(&user, "scope", start).await.unwrap()
        }));
    }
    let mut seen: Vec<i64> = Vec::new();
    for task in tasks {
        seen.push(task.await.unwrap());
    }
    seen.sort_unstable();
    assert_eq!(seen, (1..=64).collect::<Vec<_>>());
}
