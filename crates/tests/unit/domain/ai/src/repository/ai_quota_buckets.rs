// DB-backed tests for AiQuotaBucketRepository increment / upsert semantics.

use chrono::{TimeZone, Utc};
use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};

use super::{pool, user};

async fn repo() -> Option<(AiQuotaBucketRepository, systemprompt_database::DbPool)> {
    let pool = pool().await?;
    let repo = AiQuotaBucketRepository::new(&pool).expect("repo");
    Some((repo, pool))
}

#[tokio::test]
async fn increment_creates_bucket_then_accumulates() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    let window_start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().expect("ts");

    let first = repo
        .increment(IncrementParams {
            user_id: &uid,
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 1,
                input_tokens: 100,
                output_tokens: 50,
            },
        })
        .await
        .expect("increment 1");
    assert_eq!(first.requests, 1);
    assert_eq!(first.input_tokens, 100);
    assert_eq!(first.output_tokens, 50);

    let second = repo
        .increment(IncrementParams {
            user_id: &uid,
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 2,
                input_tokens: 10,
                output_tokens: 5,
            },
        })
        .await
        .expect("increment 2");
    // ON CONFLICT path adds onto the existing bucket.
    assert_eq!(second.requests, 3);
    assert_eq!(second.input_tokens, 110);
    assert_eq!(second.output_tokens, 55);
}

#[tokio::test]
async fn separate_windows_are_independent_buckets() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let uid = user();
    let email = format!("{}@ai.invalid", uid.as_str());
    systemprompt_test_fixtures::seed_user_row(&pool, &uid, &email)
        .await
        .expect("seed");
    let w1 = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).single().expect("ts");
    let w2 = Utc.with_ymd_and_hms(2026, 2, 1, 1, 0, 0).single().expect("ts");

    repo.increment(IncrementParams {
        user_id: &uid,
        window_seconds: 3600,
        window_start: w1,
        delta: QuotaBucketDelta {
            requests: 5,
            input_tokens: 0,
            output_tokens: 0,
        },
    })
    .await
    .expect("w1");
    let other = repo
        .increment(IncrementParams {
            user_id: &uid,
            window_seconds: 3600,
            window_start: w2,
            delta: QuotaBucketDelta {
                requests: 1,
                input_tokens: 0,
                output_tokens: 0,
            },
        })
        .await
        .expect("w2");
    assert_eq!(other.requests, 1);
}
