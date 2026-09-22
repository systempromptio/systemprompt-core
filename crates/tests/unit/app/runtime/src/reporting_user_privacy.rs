use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::reporting;
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_users::{UserRepository, UserStatus};

async fn fixture() -> (DisposableDb, DbPool, UserRepository) {
    let database = DisposableDb::installed("user_privacy_delivery")
        .await
        .unwrap();
    let db = database.pool().await.unwrap();
    let repository = UserRepository::new(&db).unwrap();
    (database, db, repository)
}

async fn seed(db: &DbPool, id: &str) -> UserId {
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users(id,name,email) VALUES($1,$1,$2)")
        .bind(id)
        .bind(format!("{id}@privacy.test"))
        .execute(&*pool)
        .await
        .unwrap();
    UserId::new(id)
}

async fn cleanup(database: DisposableDb, db: DbPool) {
    db.write_pool_arc().unwrap().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn status_delete_and_merge_deliver_committed_evidence_without_background_worker() {
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let source = seed(&db, "source").await;
    let target = seed(&db, "target").await;
    repository
        .update_status(&source, UserStatus::Suspended)
        .await
        .unwrap();
    let state: (bool, i64) = sqlx::query_as(
        "SELECT initialized,generation FROM analytics_projection_state WHERE singleton",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(state, (false, 0), "delivery is not a complete baseline");
    sqlx::query("INSERT INTO user_sessions(session_id,user_id) VALUES('merge-session','source')")
        .execute(&*pool)
        .await
        .unwrap();
    assert_eq!(
        repository
            .merge_users(&source, &target)
            .await
            .unwrap()
            .sessions,
        1
    );
    let owner: String = sqlx::query_scalar(
        "SELECT user_id FROM analytics_report_user_sessions WHERE session_id='merge-session'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(owner, "target");
    sqlx::query("UPDATE users SET name='pending update' WHERE id='target'")
        .execute(&*pool)
        .await
        .unwrap();
    repository.delete(&target).await.unwrap();
    let residues: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM analytics_report_users)+(SELECT count(*) FROM analytics_report_user_sessions)+(SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting')+(SELECT count(*) FROM analytics_projection_revisions)")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(residues, 0);
    cleanup(database, db).await;
}

#[tokio::test]
async fn malformed_or_over_bound_delivery_rolls_back_user_and_all_evidence() {
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "blocked").await;
    sqlx::query("UPDATE event_outbox SET fact=jsonb_set(fact,'{version}','2') WHERE consumer='analytics_reporting'")
        .execute(&*pool).await.unwrap();
    assert!(repository.delete(&user).await.is_err());
    let preserved: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM users WHERE id='blocked'),(SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL)")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(preserved, (1, 1));
    sqlx::query("UPDATE event_outbox SET fact=jsonb_set(fact,'{version}','1') WHERE consumer='analytics_reporting'")
        .execute(&*pool).await.unwrap();
    sqlx::query("INSERT INTO event_outbox(id,channel,user_id,payload,actor_kind,actor_id,origin_instance_id,consumer,fact) SELECT 'bound-'||n,channel,user_id,payload,actor_kind,actor_id,origin_instance_id,consumer,fact FROM event_outbox CROSS JOIN generate_series(1,10000) n WHERE consumer='analytics_reporting'")
        .execute(&*pool).await.unwrap();
    let error = sqlx::query("SELECT public.begin_user_privacy()")
        .execute(&*pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("54000")
    );
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(pending, 10001);
    let rollback: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM users WHERE id='blocked'),(SELECT count(*) FROM analytics_report_users WHERE id='blocked')")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(rollback, (1, 0));
    reporting::initialize(&db).await.unwrap();
    while reporting::process_pending(&db, 256).await.unwrap() != 0 {}
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}

#[tokio::test]
async fn older_pending_evidence_cannot_replace_a_newer_projected_revision() {
    use systemprompt_analytics::projection::{ReportingProjector, ReportingRow};
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "revision").await;
    reporting::initialize(&db).await.unwrap();
    reporting::process_pending(&db, 10).await.unwrap();
    sqlx::query("UPDATE users SET name='older' WHERE id='revision'")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET name='newer' WHERE id='revision'")
        .execute(&*pool)
        .await
        .unwrap();
    let newer: serde_json::Value = sqlx::query_scalar("SELECT fact->'data' FROM event_outbox WHERE processed_at IS NULL ORDER BY (fact->'data'->>'revision')::bigint DESC LIMIT 1")
        .fetch_one(&*pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    ReportingProjector::apply_fact(
        &mut tx,
        &serde_json::from_value::<ReportingRow>(newer).unwrap(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // Preparation must not replay the older pending update over that projection.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.begin_user_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    let name: String =
        sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id='revision'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(name, "newer");
    sqlx::query("SELECT public.finish_user_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}

#[tokio::test]
async fn claimed_worker_finishes_before_user_delivery_takes_projector_lock() {
    use systemprompt_analytics::projection::{self, ReportingProjector, ReportingRow};
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "claimed").await;
    reporting::initialize(&db).await.unwrap();
    reporting::process_pending(&db, 10).await.unwrap();
    sqlx::query("UPDATE users SET name='claimed update' WHERE id='claimed'")
        .execute(&*pool)
        .await
        .unwrap();
    // Same row claim and later projector order as the production durable worker.
    let mut delivery = pool.begin().await.unwrap();
    let (id, fact): (String, serde_json::Value) = sqlx::query_as("SELECT id,fact->'data' FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL FOR UPDATE SKIP LOCKED")
        .fetch_one(&mut *delivery).await.unwrap();
    let other = pool.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let mutation = tokio::spawn(async move {
        let mut tx = other.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        started.send(pid).unwrap();
        sqlx::query("SELECT public.begin_user_privacy()")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT public.finish_user_privacy()")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    });
    let pid = ready.await.unwrap();
    let waiting = wait_for_relation(&pool, pid).await;
    assert_eq!(waiting, "event_outbox");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        projection::lock_projector(&mut delivery).await.unwrap();
        ReportingProjector::apply_fact(
            &mut delivery,
            &serde_json::from_value::<ReportingRow>(fact).unwrap(),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE event_outbox SET processed_at=$2 WHERE id=$1")
            .bind(id)
            .bind(Utc::now())
            .execute(&mut *delivery)
            .await
            .unwrap();
        delivery.commit().await.unwrap();
        mutation.await.unwrap();
    })
    .await
    .expect("claimed worker and privacy delivery must both finish");
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}

async fn wait_for_relation(pool: &sqlx::PgPool, pid: i32) -> String {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let relation: Option<String> = sqlx::query_scalar("SELECT relation::regclass::text FROM pg_locks WHERE pid=$1 AND locktype='relation' AND NOT granted LIMIT 1")
                .bind(pid).fetch_optional(pool).await.unwrap();
            if let Some(relation) = relation { return relation; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("privacy reaches its expected lock barrier")
}

#[tokio::test]
async fn user_migration_before_analytics_falls_back_to_the_delivering_barrier() {
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "partial-upgrade").await;
    // Simulate users015 installed while analytics010 is not yet present: the
    // users side falls back to prepare_reporting_privacy(), which now
    // delivers the pending evidence itself instead of refusing on it.
    sqlx::query("DROP FUNCTION public.prepare_user_reporting_privacy()")
        .execute(&*pool)
        .await
        .unwrap();
    let mut probe = pool.begin().await.unwrap();
    sqlx::query("SELECT public.begin_user_privacy()")
        .execute(&mut *probe)
        .await
        .unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL")
        .fetch_one(&mut *probe).await.unwrap();
    assert_eq!(pending, 0);
    probe.rollback().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(include_str!(
        "../../../../../domain/analytics/schema/reporting_privacy.sql"
    )))
    .execute(&*pool)
    .await
    .unwrap();
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}

#[tokio::test]
async fn source_writer_commits_before_user_delivery_captures_its_evidence() {
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "source-writer").await;
    let mut writer = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO user_sessions(session_id,user_id) VALUES('pending-source','source-writer')",
    )
    .execute(&mut *writer)
    .await
    .unwrap();
    let other = pool.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let mutation = tokio::spawn(async move {
        let mut tx = other.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        started.send(pid).unwrap();
        sqlx::query("SELECT public.begin_user_privacy()")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT public.finish_user_privacy()")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    });
    let relation = wait_for_relation(&pool, ready.await.unwrap()).await;
    assert_eq!(relation, "user_sessions");
    writer.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), mutation)
        .await
        .unwrap()
        .unwrap();
    let owner: String = sqlx::query_scalar(
        "SELECT user_id FROM analytics_report_user_sessions WHERE session_id='pending-source'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(owner, "source-writer");
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}

#[tokio::test]
async fn user_delivery_rejects_the_same_typed_fact_corruption_as_the_worker() {
    let (database, db, repository) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let user = seed(&db, "typed-contract").await;
    reporting::initialize(&db).await.unwrap();
    let original: serde_json::Value = sqlx::query_scalar("SELECT fact FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL")
        .fetch_one(&*pool).await.unwrap();
    for corruption in ["extra", "numeric-key", "string-revision", "string-version"] {
        let mut malformed = original.clone();
        match corruption {
            "extra" => malformed["data"]["unexpected"] = serde_json::json!(true),
            "numeric-key" => malformed["data"]["key"] = serde_json::json!(7),
            "string-revision" => malformed["data"]["revision"] = serde_json::json!("1"),
            "string-version" => malformed["version"] = serde_json::json!("1"),
            _ => unreachable!(),
        }
        sqlx::query("UPDATE event_outbox SET fact=$1 WHERE consumer='analytics_reporting'")
            .bind(malformed.clone())
            .execute(&*pool)
            .await
            .unwrap();
        assert!(
            reporting::process_pending(&db, 10).await.is_err(),
            "{corruption}"
        );
        assert!(repository.delete(&user).await.is_err(), "{corruption}");
        let preserved: serde_json::Value = sqlx::query_scalar("SELECT fact FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL")
            .fetch_one(&*pool).await.unwrap();
        assert_eq!(preserved, malformed);
        let present: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE id='typed-contract'")
                .fetch_one(&*pool)
                .await
                .unwrap();
        assert_eq!(present, 1);
    }
    sqlx::query("UPDATE event_outbox SET fact=$1 WHERE consumer='analytics_reporting'")
        .bind(original)
        .execute(&*pool)
        .await
        .unwrap();
    repository.delete(&user).await.unwrap();
    cleanup(database, db).await;
}
