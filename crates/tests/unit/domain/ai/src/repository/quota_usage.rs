// DB-backed tests for QuotaUsageRepository: window usage summed over shards.

use chrono::{TimeZone, Utc};
use systemprompt_ai::repository::{
    AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta, QuotaUsageRepository,
    QuotaWindowKey,
};

use super::bootstrapped_pool;

fn subject() -> String {
    format!("project-{}", uuid::Uuid::new_v4().simple())
}

fn kind() -> String {
    format!("usage-test-{}", uuid::Uuid::new_v4().simple())
}

fn delta(requests: i64, cost: i64) -> QuotaBucketDelta {
    QuotaBucketDelta {
        requests,
        input_tokens: requests * 10,
        output_tokens: requests * 5,
        cost_microdollars: cost,
    }
}

#[tokio::test]
async fn usage_sums_every_shard_of_one_window() {
    let pool = bootstrapped_pool().await;
    let buckets = AiQuotaBucketRepository::new(&pool);
    let usage = QuotaUsageRepository::new(&pool);
    let kind = kind();
    let subject = subject();
    let window_start = Utc
        .with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
        .single()
        .expect("ts");
    let window = QuotaWindowKey {
        subject_kind: &kind,
        window_seconds: 86_400,
        window_start,
    };
    for (shard, requests) in [(0_i16, 2_i64), (1, 3), (5, 4)] {
        buckets
            .increment(IncrementParams {
                subject_kind: &kind,
                subject_id: &subject,
                window_seconds: window.window_seconds,
                window_start,
                shard,
                delta: delta(requests, requests * 100),
            })
            .await
            .expect("increment");
    }
    buckets
        .increment(IncrementParams {
            subject_kind: &kind,
            subject_id: &subject,
            window_seconds: window.window_seconds,
            window_start: window_start + chrono::Duration::days(1),
            shard: 0,
            delta: delta(50, 5_000),
        })
        .await
        .expect("next window");

    let total = usage.usage(window, &subject).await.expect("usage");
    assert_eq!(total.requests, 9);
    assert_eq!(total.input_tokens, 90);
    assert_eq!(total.output_tokens, 45);
    assert_eq!(total.cost_microdollars, 900);
}

#[tokio::test]
async fn usage_of_an_untouched_window_is_zero() {
    let pool = bootstrapped_pool().await;
    let usage = QuotaUsageRepository::new(&pool);
    let kind = kind();
    let total = usage
        .usage(
            QuotaWindowKey {
                subject_kind: &kind,
                window_seconds: 3600,
                window_start: Utc
                    .with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
                    .single()
                    .expect("ts"),
            },
            &subject(),
        )
        .await
        .expect("usage");
    assert_eq!(total.requests, 0);
    assert_eq!(total.cost_microdollars, 0);
}

#[tokio::test]
async fn window_usage_lists_subjects_by_cost_and_honours_the_limit() {
    let pool = bootstrapped_pool().await;
    let buckets = AiQuotaBucketRepository::new(&pool);
    let usage = QuotaUsageRepository::new(&pool);
    let kind = kind();
    let window_start = Utc
        .with_ymd_and_hms(2026, 3, 2, 0, 0, 0)
        .single()
        .expect("ts");
    let window = QuotaWindowKey {
        subject_kind: &kind,
        window_seconds: 3600,
        window_start,
    };
    let cheap = subject();
    let dear = subject();
    for (subject, shard, cost) in [(&cheap, 0_i16, 100_i64), (&dear, 0, 400), (&dear, 3, 600)] {
        buckets
            .increment(IncrementParams {
                subject_kind: &kind,
                subject_id: subject,
                window_seconds: window.window_seconds,
                window_start,
                shard,
                delta: delta(1, cost),
            })
            .await
            .expect("increment");
    }

    let listed = usage.window_usage(window, 10).await.expect("window usage");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].subject_id, dear);
    assert_eq!(listed[0].usage.cost_microdollars, 1_000);
    assert_eq!(listed[0].usage.requests, 2);
    assert_eq!(listed[1].subject_id, cheap);

    let top = usage.window_usage(window, 1).await.expect("window usage");
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].subject_id, dear);
}
