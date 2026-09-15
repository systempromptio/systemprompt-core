use super::*;

#[tokio::test]
async fn committed_preinitialization_evidence_requires_real_baseline_and_delivery_before_expiry() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query(
        "INSERT INTO users(id,name,email) VALUES('pending','pending','pending@example.test')",
    )
    .execute(&*pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_sessions(session_id,user_id,last_activity_at,ended_at,expires_at,ip_address) VALUES('pending-session','pending','2020-01-01','2020-01-02','2020-01-02','192.0.2.41')")
        .execute(&*pool).await.unwrap();
    let error = sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&*pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55000")
    );
    let retained: (bool, i64, i64) = sqlx::query_as("SELECT initialized,(SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL),(SELECT count(*) FROM user_sessions WHERE session_id='pending-session') FROM analytics_projection_state WHERE singleton")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(retained, (false, 2, 1));
    initialize_and_drain(&db, 2).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    let cutoff = Utc::now() - Duration::days(90);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT public.expire_reporting_sessions($1)")
            .bind(cutoff)
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        1
    );
    sqlx::query("SELECT public.finish_reporting_privacy($1)")
        .bind(cutoff)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let residues: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting')+(SELECT count(*) FROM analytics_report_user_sessions)+(SELECT count(*) FROM user_sessions)")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(residues, 0);
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn owners_compacting_valid_clocks_out_of_order_preserve_global_privacy_and_local_clock_checks()
 {
    use systemprompt_analytics::snapshots::FeedbackSnapshotsRepository;
    use systemprompt_identifiers::UserId;
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    for script in [
        include_str!("../../../../../domain/analytics/schema/ingestion_producers.sql").to_owned(),
        crate::reporting::analytics_schema_sql(|table| {
            table.starts_with("analytics_fact_")
                || table.starts_with("analytics_normalized_")
                || table.starts_with("analytics_feedback_")
                || table.starts_with("analytics_snapshot")
        }),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(script))
            .execute(&*pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO users(id,name,email) VALUES('first','first','first@example.test'),('second','second','second@example.test')")
        .execute(&*pool).await.unwrap();
    sqlx::query("INSERT INTO ai_requests(id,request_id,user_id,context_id,provider,model,actor_kind,actor_id,created_at) VALUES('first-old','first-old','first','ctx','fixture','fixture','job','fixture','2020-01-01'),('second-old','second-old','second','ctx','fixture','fixture','job','fixture','2020-01-01')")
        .execute(&*pool).await.unwrap();
    initialize_and_drain(&db, 4).await;
    for owner in ["first", "second"] {
        sqlx::query("INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1)")
            .bind(owner)
            .execute(&*pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO analytics_snapshot_state(owner_id) VALUES($1)")
            .bind(owner)
            .execute(&*pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO analytics_fact_consumers(owner_id,consumer) VALUES($1,'snapshots-v1')",
        )
        .bind(owner)
        .execute(&*pool)
        .await
        .unwrap();
    }
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
    for (owner, clock) in [("first", now), ("second", now - Duration::days(1))] {
        let mut tx = pool.begin().await.unwrap();
        FeedbackSnapshotsRepository::compact_in(&mut tx, &UserId::new(owner), clock)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    let cutoff: chrono::DateTime<Utc> = sqlx::query_scalar(
        "SELECT evidence_cutoff FROM analytics_projection_state WHERE singleton",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(cutoff, now - Duration::days(90));
    let copied: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_report_ai_requests")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(copied, 0);
    reporting::rebuild(&db).await.unwrap();
    let copied: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_report_ai_requests")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(
        copied, 0,
        "older source rows cannot rebuild after either owner's compaction"
    );
    let mut tx = pool.begin().await.unwrap();
    assert!(
        FeedbackSnapshotsRepository::compact_in(
            &mut tx,
            &UserId::new("second"),
            now - Duration::days(1) - Duration::seconds(1)
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::query("SELECT public.finish_reporting_compaction($1)")
        .bind(Utc::now() + Duration::days(1))
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("22023")
    );
    tx.rollback().await.unwrap();
    cleanup(&admin, &pool, &database).await;
}
