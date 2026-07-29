// DB-backed tests for AiQuotaBucketRepository increment / upsert semantics.

use chrono::{TimeZone, Utc};
use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};

use super::{pool, user};

async fn repo() -> Option<(AiQuotaBucketRepository, systemprompt_database::DbPool)> {
    let pool = pool().await?;
    let repo = AiQuotaBucketRepository::new(&pool).expect("repo");
    Some((repo, pool))
}

const NO_DELTA: QuotaBucketDelta = QuotaBucketDelta {
    requests: 0,
    input_tokens: 0,
    output_tokens: 0,
    cost_microdollars: 0,
};

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
    let window_start = Utc
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("ts");

    let first = repo
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: uid.as_str(),
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 1,
                input_tokens: 100,
                output_tokens: 50,
                cost_microdollars: 700,
            },
        })
        .await
        .expect("increment 1");
    assert_eq!(first.requests, 1);
    assert_eq!(first.input_tokens, 100);
    assert_eq!(first.output_tokens, 50);
    assert_eq!(first.cost_microdollars, 700);

    let second = repo
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: uid.as_str(),
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 2,
                input_tokens: 10,
                output_tokens: 5,
                cost_microdollars: 300,
            },
        })
        .await
        .expect("increment 2");
    // ON CONFLICT path adds onto the existing bucket.
    assert_eq!(second.requests, 3);
    assert_eq!(second.input_tokens, 110);
    assert_eq!(second.output_tokens, 55);
    assert_eq!(second.cost_microdollars, 1000);
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
    let w1 = Utc
        .with_ymd_and_hms(2026, 2, 1, 0, 0, 0)
        .single()
        .expect("ts");
    let w2 = Utc
        .with_ymd_and_hms(2026, 2, 1, 1, 0, 0)
        .single()
        .expect("ts");

    repo.increment(IncrementParams {
        subject_kind: "user",
        subject_id: uid.as_str(),
        window_seconds: 3600,
        window_start: w1,
        delta: QuotaBucketDelta {
            requests: 5,
            ..NO_DELTA
        },
    })
    .await
    .expect("w1");
    let other = repo
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: uid.as_str(),
            window_seconds: 3600,
            window_start: w2,
            delta: QuotaBucketDelta {
                requests: 1,
                ..NO_DELTA
            },
        })
        .await
        .expect("w2");
    assert_eq!(other.requests, 1);
}

#[tokio::test]
async fn the_same_subject_id_under_different_kinds_is_two_buckets() {
    let Some((repo, _pool)) = repo().await else {
        return;
    };
    let subject = format!("shared-{}", uuid::Uuid::new_v4());
    let window_start = Utc
        .with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
        .single()
        .expect("ts");

    let as_user = repo
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: &subject,
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 4,
                ..NO_DELTA
            },
        })
        .await
        .expect("user bucket");
    let as_org = repo
        .increment(IncrementParams {
            subject_kind: "organization",
            subject_id: &subject,
            window_seconds: 3600,
            window_start,
            delta: QuotaBucketDelta {
                requests: 1,
                ..NO_DELTA
            },
        })
        .await
        .expect("org bucket");

    assert_eq!(as_user.requests, 4);
    assert_eq!(
        as_org.requests, 1,
        "an organization bucket must not collide with a user bucket sharing its id"
    );
}
