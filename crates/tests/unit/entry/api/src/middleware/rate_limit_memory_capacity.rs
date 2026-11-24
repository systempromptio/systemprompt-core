use std::num::NonZeroU64;

use chrono::Utc;
use systemprompt_api::services::middleware::MemoryRateLimitBuckets;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RateLimitBuckets;

#[tokio::test]
async fn the_profile_capacity_bounds_the_callers_tracked() {
    let capacity = 64;
    let buckets =
        MemoryRateLimitBuckets::with_capacity(NonZeroU64::new(capacity).expect("non-zero"));
    let window = Utc::now();
    for caller in 0..1000 {
        let _charged = buckets
            .hit(&UserId::new(format!("caller-{caller}")), "scope", window)
            .await;
    }
    assert!(buckets.tracked() as u64 <= capacity);
    assert!(
        buckets.refusals() > 0,
        "a full shard refuses rather than evicts in-window"
    );
}

#[tokio::test]
async fn a_capacity_below_the_shard_count_keeps_one_caller_per_shard() {
    let buckets = MemoryRateLimitBuckets::with_capacity(NonZeroU64::MIN);
    let charged = buckets
        .hit(&UserId::new("only"), "scope", Utc::now())
        .await
        .expect("one caller fits");
    assert_eq!(charged, 1);
}
