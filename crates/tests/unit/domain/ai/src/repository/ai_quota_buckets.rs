// DB-backed tests for AiQuotaBucketRepository increment / upsert semantics.

use chrono::{TimeZone, Utc};
use systemprompt_ai::repository::{AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta};

use super::{bootstrapped_pool, user};

async fn test_repo() -> (AiQuotaBucketRepository, systemprompt_database::DbPool) {
    let pool = bootstrapped_pool().await;
    let repo = AiQuotaBucketRepository::new(&pool);
    (repo, pool)
}

const NO_DELTA: QuotaBucketDelta = QuotaBucketDelta {
    requests: 0,
    input_tokens: 0,
    output_tokens: 0,
    cost_microdollars: 0,
};

#[tokio::test]
async fn increment_creates_bucket_then_accumulates() {
    let (repo, pool) = test_repo().await;
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
            shard: 0,
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
            shard: 0,
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
    let (repo, pool) = test_repo().await;
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
        shard: 0,
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
            shard: 0,
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
    let (repo, _pool) = test_repo().await;
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
            shard: 0,
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
            shard: 0,
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

fn org_increment(subject: &str, shard: i16, requests: i64, cost: i64) -> IncrementParams<'_> {
    IncrementParams {
        subject_kind: "organization",
        subject_id: subject,
        window_seconds: 3600,
        window_start: Utc
            .with_ymd_and_hms(2026, 4, 1, 0, 0, 0)
            .single()
            .expect("ts"),
        shard,
        delta: QuotaBucketDelta {
            requests,
            cost_microdollars: cost,
            ..NO_DELTA
        },
    }
}

#[tokio::test]
async fn increments_on_different_shards_report_the_window_total() {
    let (repo, pool) = test_repo().await;
    let subject = format!("org-{}", uuid::Uuid::new_v4());

    let first = repo
        .increment(org_increment(&subject, 0, 1, 100))
        .await
        .expect("shard 0");
    let second = repo
        .increment(org_increment(&subject, 3, 1, 250))
        .await
        .expect("shard 3");
    let third = repo
        .increment(org_increment(&subject, 3, 2, 50))
        .await
        .expect("shard 3 again");

    assert_eq!(first.requests, 1);
    assert_eq!(second.requests, 2);
    assert_eq!(second.cost_microdollars, 350);
    assert_eq!(third.requests, 4);
    assert_eq!(third.cost_microdollars, 400);

    let rows = sqlx::query!(
        r#"SELECT shard, requests FROM ai_quota_buckets
           WHERE subject_kind = 'organization' AND subject_id = $1 ORDER BY shard"#,
        subject
    )
    .fetch_all(pool.pool().as_ref())
    .await
    .expect("rows");
    let shards: Vec<(i16, i64)> = rows.iter().map(|r| (r.shard, r.requests)).collect();
    assert_eq!(shards, vec![(0, 1), (3, 3)]);
}

#[tokio::test]
async fn a_settlement_on_its_shard_trues_up_the_window_total() {
    let (repo, _pool) = test_repo().await;
    let subject = format!("org-{}", uuid::Uuid::new_v4());

    repo.increment(org_increment(&subject, 1, 1, 1_000))
        .await
        .expect("reserve shard 1");
    repo.increment(org_increment(&subject, 5, 1, 1_000))
        .await
        .expect("reserve shard 5");
    let settled = repo
        .increment(org_increment(&subject, 5, 0, -700))
        .await
        .expect("settle shard 5");

    assert_eq!(settled.requests, 2);
    assert_eq!(settled.cost_microdollars, 1_300);
}

#[tokio::test]
async fn concurrent_sharded_reservations_lose_no_increment() {
    let (repo, _pool) = test_repo().await;
    let subject = format!("org-{}", uuid::Uuid::new_v4());
    let tasks: Vec<_> = (0..32_i16)
        .map(|i| {
            let repo = repo.clone();
            let subject = subject.clone();
            tokio::spawn(async move {
                repo.increment(org_increment(&subject, i % 8, 1, 10))
                    .await
                    .expect("increment")
            })
        })
        .collect();
    for task in tasks {
        task.await.expect("join");
    }
    let total = repo
        .increment(org_increment(&subject, 0, 0, 0))
        .await
        .expect("read total");
    assert_eq!(total.requests, 32);
    assert_eq!(total.cost_microdollars, 320);
}
