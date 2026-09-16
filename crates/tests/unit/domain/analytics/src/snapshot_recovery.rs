use super::*;
use systemprompt_analytics::feedback::DeltaClaim;
use systemprompt_analytics::snapshots::SnapshotJobState;
use systemprompt_identifiers::{AnalyticsSnapshotJobId, AnalyticsWorkerId};

#[tokio::test]
async fn stale_delta_lease_is_fenced_and_restarted_worker_applies_once() {
    let f = Fixture::new().await;
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("request", 1, now, 9))
        .await
        .unwrap();
    f.drain().await;
    let stale = f
        .repository
        .claim_deltas(
            &f.owner,
            "snapshots-v1",
            &AnalyticsWorkerId::generate(),
            DeltaClaim {
                limit: 256,
                lease_seconds: 300,
            },
        )
        .await
        .unwrap()
        .unwrap();
    sqlx::query!("UPDATE analytics_fact_consumers SET lease_until=clock_timestamp()-interval '1 second' WHERE owner_id=$1",f.owner.as_str()).execute(&f.pool).await.unwrap();
    let fresh = f
        .repository
        .claim_deltas(
            &f.owner,
            "snapshots-v1",
            &AnalyticsWorkerId::generate(),
            DeltaClaim {
                limit: 256,
                lease_seconds: 300,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        repository(&f)
            .apply_batch(&f.owner, &stale, now)
            .await
            .is_err()
    );
    repository(&f)
        .apply_batch(&f.owner, &fresh, now)
        .await
        .unwrap();
    assert!(
        repository(&f)
            .apply_batch(&f.owner, &fresh, now)
            .await
            .is_err()
    );
    assert_eq!(refresh(&f, now).await.metrics.requests, 1);
}

#[tokio::test]
async fn concurrent_workers_and_reordered_replacements_match_latest_revision() {
    let f = Fixture::new().await;
    let now = Utc::now();
    for revision in [3, 1, 2] {
        f.repository
            .submit(&f.owner, &request("same", revision, now, revision * 10))
            .await
            .unwrap();
    }
    f.drain().await;
    let a = repository(&f);
    let b = repository(&f);
    let worker_a = AnalyticsWorkerId::generate();
    let worker_b = AnalyticsWorkerId::generate();
    let (x, y) = tokio::join!(
        a.process(&f.owner, &worker_a, now),
        b.process(&f.owner, &worker_b, now)
    );
    assert!(x.is_ok());
    assert!(y.is_ok());
    let snapshot = refresh(&f, now).await;
    assert_eq!(snapshot.metrics.requests, 1);
    assert_eq!(snapshot.spend_by_currency["USD"], 30);
}

#[tokio::test]
async fn custom_jobs_are_idempotent_fenced_and_invalidated_by_corrections() {
    let f = Fixture::new().await;
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("request", 1, now, 10))
        .await
        .unwrap();
    refresh(&f, now).await;
    let request = SnapshotRangeRequest {
        operation_id: AnalyticsSnapshotJobId::generate(),
        resource_id: None,
        from_day: now.date_naive(),
        to_day: now.date_naive() + Duration::days(1),
    };
    let repo = repository(&f);
    repo.request_range(&f.owner, &request, now).await.unwrap();
    repo.request_range(&f.owner, &request, now).await.unwrap();
    let mut conflict = request.clone();
    conflict.from_day -= Duration::days(1);
    assert!(repo.request_range(&f.owner, &conflict, now).await.is_err());
    let lease = repo
        .claim_range(&f.owner, &AnalyticsWorkerId::generate())
        .await
        .unwrap()
        .unwrap();
    repo.complete_range(&f.owner, &lease, now).await.unwrap();
    assert_eq!(
        repo.range_job(&f.owner, &request.operation_id)
            .await
            .unwrap()
            .unwrap()
            .result
            .unwrap()
            .metrics
            .requests,
        1
    );
    f.repository
        .submit(&f.owner, &super::request("request", 2, now, 20))
        .await
        .unwrap();
    refresh(&f, now).await;
    let job = repo
        .range_job(&f.owner, &request.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.state, SnapshotJobState::Pending);
    assert!(job.result.is_none());
    assert!(repo.complete_range(&f.owner, &lease, now).await.is_err());
}

#[tokio::test]
async fn committed_evidence_to_snapshot_functional_freshness() {
    let f = Fixture::new().await;
    let start = std::time::Instant::now();
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("fresh", 1, now, 1))
        .await
        .unwrap();
    let snapshot = refresh(&f, now).await;
    assert_eq!(snapshot.metrics.requests, 1);
    assert!(snapshot.fact_generation > 0);
    eprintln!(
        "functional freshness: one committed request, local PostgreSQL, direct worker drain, {:?}; no throughput or p95 claim",
        start.elapsed()
    );
}

#[tokio::test]
async fn assembly_error_fails_the_job_and_it_is_not_reclaimed() {
    let f = Fixture::new().await;
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("request", 1, now, 10))
        .await
        .unwrap();
    refresh(&f, now).await;
    sqlx::query!(
        "UPDATE analytics_snapshot_daily SET spend='{\"USD\":\"not-a-number\"}'::jsonb WHERE owner_id=$1",
        f.owner.as_str()
    )
    .execute(&f.pool)
    .await
    .unwrap();
    let request = SnapshotRangeRequest {
        operation_id: AnalyticsSnapshotJobId::generate(),
        resource_id: None,
        from_day: now.date_naive(),
        to_day: now.date_naive() + Duration::days(1),
    };
    let repo = repository(&f);
    repo.request_range(&f.owner, &request, now).await.unwrap();
    let lease = repo
        .claim_range(&f.owner, &AnalyticsWorkerId::generate())
        .await
        .unwrap()
        .unwrap();
    assert!(repo.complete_range(&f.owner, &lease, now).await.is_err());
    let job = repo
        .range_job(&f.owner, &request.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.state, SnapshotJobState::Failed);
    assert!(job.result.is_none());
    assert!(
        job.diagnostic
            .as_deref()
            .is_some_and(|d| d.contains("Invalid recorded spend")),
        "got: {:?}",
        job.diagnostic
    );
    assert!(
        repo.claim_range(&f.owner, &AnalyticsWorkerId::generate())
            .await
            .unwrap()
            .is_none()
    );
    assert!(repo.fail_range(&f.owner, &lease, "late").await.is_err());
}
