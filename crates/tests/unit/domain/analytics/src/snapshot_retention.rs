use super::*;
use systemprompt_test_fixtures::{DisposableDb, drain_reporting};

async fn isolated() -> (DisposableDb, systemprompt_database::DbPool, Fixture) {
    let database = DisposableDb::installed("feedback_retention").await.unwrap();
    let db = database.pool().await.unwrap();
    let fixture = Fixture::in_db(&db).await;
    drain_reporting(&db).await.unwrap();
    (database, db, fixture)
}

async fn cleanup(database: DisposableDb, db: systemprompt_database::DbPool) {
    db.write_pool_arc().unwrap().close().await;
    database.drop_now().await;
}

async fn compact(
    f: &Fixture,
    now: chrono::DateTime<Utc>,
) -> systemprompt_analytics::Result<systemprompt_analytics::snapshots::RetentionOutcome> {
    let mut tx = f.pool.begin().await?;
    let result = FeedbackSnapshotsRepository::compact_in(&mut tx, &f.owner, now).await?;
    tx.commit().await?;
    Ok(result)
}

#[tokio::test]
async fn pending_producer_or_fact_changes_prevent_retention() {
    let (database, db, f) = isolated().await;
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("ready", 1, now, 1))
        .await
        .unwrap();
    refresh(&f, now).await;
    let producer = format!("fixture-{}", f.owner);
    sqlx::query!("INSERT INTO analytics_ingestion_producers(producer,pending_count,oldest_pending_at) VALUES($1,1,clock_timestamp())",producer).execute(&f.pool).await.unwrap();
    assert!(compact(&f, now).await.is_err());
    sqlx::query!(
        "DELETE FROM analytics_ingestion_producers WHERE producer=$1",
        producer
    )
    .execute(&f.pool)
    .await
    .unwrap();
    f.repository
        .submit(&f.owner, &request("pending", 1, now, 2))
        .await
        .unwrap();
    assert!(compact(&f, now).await.is_err());
    refresh(&f, now).await;
    assert!(compact(&f, now).await.is_ok());
    cleanup(database, db).await;
}

#[tokio::test]
async fn retention_erases_evidence_and_preserves_only_safe_daily_totals_then_suppresses_late_correction()
 {
    let (database, db, f) = isolated().await;
    let now = Utc::now();
    let old = now - Duration::days(100);
    let unsafe_day = now - Duration::days(110);
    for index in 0..5 {
        let mut change = request(&format!("old-{index}"), 1, old, 10);
        identify(&mut change, &format!("old-user-{index}"));
        f.repository.submit(&f.owner, &change).await.unwrap();
    }
    let mut unsafe_change = request("unsafe", 1, unsafe_day, 20);
    identify(&mut unsafe_change, "lonely-user");
    f.repository.submit(&f.owner, &unsafe_change).await.unwrap();
    refresh(&f, now).await;
    let before = repository(&f)
        .snapshot(&f.owner, None, 365)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.metrics.requests, 6);
    assert!(before.distinct_users.is_none());
    let outcome = compact(&f, now).await.unwrap();
    assert_eq!(outcome.removed_facts, 6);
    let counts=sqlx::query!(r#"SELECT (SELECT COUNT(*) FROM analytics_normalized_facts WHERE owner_id=$1) AS "facts!",(SELECT COUNT(*) FROM analytics_fact_changes WHERE owner_id=$1) AS "changes!",(SELECT COUNT(*) FROM analytics_fact_deltas WHERE owner_id=$1) AS "deltas!",(SELECT COUNT(*) FROM analytics_snapshot_shadow WHERE owner_id=$1) AS "shadow!",(SELECT COUNT(*) FROM analytics_snapshot_identities WHERE owner_id=$1) AS "identities!""#,f.owner.as_str()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        (
            counts.facts,
            counts.changes,
            counts.deltas,
            counts.shadow,
            counts.identities
        ),
        (0, 0, 0, 0, 0)
    );
    let retained = repository(&f)
        .snapshot(&f.owner, None, 365)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.metrics.requests, 5);
    assert_eq!(retained.suppressed_days, 1);
    assert!(retained.distinct_users.is_none());
    let mut correction = request("old-0", 2, old, 10);
    correction.operation = AnalyticsChangeOperation::Tombstone;
    f.repository.submit(&f.owner, &correction).await.unwrap();
    refresh(&f, now).await;
    let corrected = repository(&f)
        .snapshot(&f.owner, None, 365)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(corrected.metrics.requests, 0);
    assert_eq!(corrected.suppressed_days, 2);
    assert!(corrected.generation > retained.generation);
    cleanup(database, db).await;
}

#[tokio::test]
async fn retention_blocks_new_producer_registration_until_transaction_finishes() {
    let (database, db, f) = isolated().await;
    let now = Utc::now();
    f.repository
        .submit(&f.owner, &request("ready", 1, now, 1))
        .await
        .unwrap();
    refresh(&f, now).await;
    let mut tx = f.pool.begin().await.unwrap();
    FeedbackSnapshotsRepository::compact_in(&mut tx, &f.owner, now)
        .await
        .unwrap();
    let producer = format!("late-{}", f.owner);
    let pool = f.pool.clone();
    let name = producer.clone();
    let task = tokio::spawn(async move {
        sqlx::query!(
            "INSERT INTO analytics_ingestion_producers(producer) VALUES($1)",
            name
        )
        .execute(&pool)
        .await
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    tx.commit().await.unwrap();
    task.await.unwrap().unwrap();
    sqlx::query!(
        "DELETE FROM analytics_ingestion_producers WHERE producer=$1",
        producer
    )
    .execute(&f.pool)
    .await
    .unwrap();
    cleanup(database, db).await;
}

#[tokio::test]
async fn midday_cutoff_preserves_younger_evidence_and_rejects_clock_regression() {
    let (database, db, f) = isolated().await;
    let now = (Utc::now().date_naive() - Duration::days(1))
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc();
    let cutoff = now - Duration::days(90);
    f.repository
        .submit(
            &f.owner,
            &request("older", 1, cutoff - Duration::hours(1), 1),
        )
        .await
        .unwrap();
    f.repository
        .submit(
            &f.owner,
            &request("younger", 1, cutoff + Duration::hours(1), 1),
        )
        .await
        .unwrap();
    refresh(&f, now).await;
    compact(&f, now).await.unwrap();
    assert!(
        f.repository
            .get_fact(&f.owner, &key(AnalyticsFactKind::Request, "older"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        f.repository
            .get_fact(&f.owner, &key(AnalyticsFactKind::Request, "younger"))
            .await
            .unwrap()
            .is_some()
    );
    assert!(compact(&f, now - Duration::seconds(1)).await.is_err());
    assert!(compact(&f, Utc::now() + Duration::days(1)).await.is_err());
    cleanup(database, db).await;
}

#[tokio::test]
async fn recent_association_cannot_retain_an_expired_request_identity() {
    let (database, db, f) = isolated().await;
    let now = Utc::now();
    let resource = ManagedResourceId::generate();
    f.repository
        .submit(
            &f.owner,
            &request("expired", 1, now - Duration::days(100), 1),
        )
        .await
        .unwrap();
    f.repository
        .submit(
            &f.owner,
            &association("recent-link", "expired", &resource, now),
        )
        .await
        .unwrap();
    refresh(&f, now).await;
    let outcome = compact(&f, now).await.unwrap();
    assert_eq!(outcome.removed_facts, 2);
    assert!(
        f.repository
            .get_fact(
                &f.owner,
                &key(AnalyticsFactKind::ResourceAssociation, "recent-link")
            )
            .await
            .unwrap()
            .is_none()
    );
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM analytics_fact_changes WHERE owner_id=$1"#,
        f.owner.as_str()
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    cleanup(database, db).await;
}

#[tokio::test]
async fn all_owner_compaction_advances_both_cutoffs_before_global_expiry() {
    let (database, db, a) = isolated().await;
    let b = Fixture::in_db(&db).await;
    drain_reporting(&db).await.unwrap();
    let now = Utc::now();
    for f in [&a, &b] {
        f.repository
            .submit(&f.owner, &request("old", 1, now - Duration::days(100), 1))
            .await
            .unwrap();
        refresh(f, now).await;
    }
    let mut tx = a.pool.begin().await.unwrap();
    let owners = vec![a.owner.as_str().to_owned(), b.owner.as_str().to_owned()];
    let summary = FeedbackSnapshotsRepository::compact_all_in(&mut tx, now)
        .await
        .unwrap();
    assert_eq!(summary.organizations, 2);
    assert_eq!(summary.removed_facts, 2);
    let prepared=sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM analytics_snapshot_state WHERE owner_id=ANY($1) AND evidence_cutoff>=$2"#,&owners,now-Duration::days(90)).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(prepared, 2);
    tx.commit().await.unwrap();
    cleanup(database, db).await;
}

#[path = "snapshot_retention_lock_order.rs"]
mod lock_order;
