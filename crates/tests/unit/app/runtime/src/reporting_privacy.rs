use super::{fixture, initialize_and_drain};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::time::Duration as Timeout;
use systemprompt_runtime::reporting;

async fn cleanup(admin: &PgPool, pool: &PgPool, database: &str) {
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {database} WITH (FORCE)"
    )))
    .execute(admin)
    .await
    .unwrap();
}

async fn wait_for_relation(pool: &PgPool, pid: i32) -> String {
    tokio::time::timeout(Timeout::from_secs(10), async {
        loop {
            let waiting: Option<String> = sqlx::query_scalar(
                "SELECT relation::regclass::text FROM pg_locks WHERE pid=$1 AND locktype='relation' AND NOT granted LIMIT 1",
            ).bind(pid).fetch_optional(pool).await.unwrap();
            if let Some(table) = waiting {
                return table;
            }
            tokio::time::sleep(Timeout::from_millis(10)).await;
        }
    }).await.expect("privacy must reach its source barrier")
}

#[tokio::test]
async fn claimed_reporting_evidence_blocks_privacy_without_waiting_for_its_row_lock() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query(
        "INSERT INTO users(id,name,email) VALUES('privacy','before','privacy@example.test')",
    )
    .execute(&*pool)
    .await
    .unwrap();
    initialize_and_drain(&db, 1).await;
    sqlx::query("UPDATE users SET name='committed' WHERE id='privacy'")
        .execute(&*pool)
        .await
        .unwrap();
    let mut claim = pool.begin().await.unwrap();
    let event: String = sqlx::query_scalar("SELECT id FROM event_outbox WHERE consumer='analytics_reporting' AND processed_at IS NULL FOR UPDATE")
        .fetch_one(&mut *claim).await.unwrap();
    let mut privacy = pool.begin().await.unwrap();
    let error = tokio::time::timeout(
        Timeout::from_secs(3),
        sqlx::query("SELECT public.prepare_reporting_privacy()").execute(&mut *privacy),
    )
    .await
    .expect("pending evidence must reject without waiting on the claimed row")
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55000")
    );
    privacy.rollback().await.unwrap();
    let pending: bool =
        sqlx::query_scalar("SELECT processed_at IS NULL FROM event_outbox WHERE id=$1")
            .bind(&event)
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert!(pending);
    claim.rollback().await.unwrap();
    assert_eq!(reporting::process_pending(&db, 10).await.unwrap(), 1);
    let mut privacy = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *privacy)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET name='atomic' WHERE id='privacy'")
        .execute(&mut *privacy)
        .await
        .unwrap();
    let processed: i64 = sqlx::query_scalar("SELECT public.finish_reporting_privacy(NULL)")
        .fetch_one(&mut *privacy)
        .await
        .unwrap();
    assert_eq!(processed, 1);
    privacy.commit().await.unwrap();
    let name: String =
        sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id='privacy'")
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert_eq!(name, "atomic");
    let residues: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting') + (SELECT count(*) FROM analytics_projection_revisions)")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(residues, 0);
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn privacy_source_barrier_allows_a_preexisting_writer_to_finish_its_user_foreign_key() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users(id,name,email) VALUES('fk','fk','fk@example.test')")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE user_sessions ADD FOREIGN KEY(user_id) REFERENCES users(id)")
        .execute(&*pool)
        .await
        .unwrap();
    initialize_and_drain(&db, 1).await;
    let mut writer = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE user_sessions IN ROW EXCLUSIVE MODE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let other_pool = pool.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let privacy = tokio::spawn(async move {
        let mut tx = other_pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        started.send(pid).unwrap();
        sqlx::query("SELECT public.prepare_reporting_privacy()")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
    });
    let pid = ready.await.unwrap();
    assert_eq!(wait_for_relation(&pool, pid).await, "user_sessions");
    let mode: String = sqlx::query_scalar("SELECT mode FROM pg_locks WHERE pid=$1 AND relation='users'::regclass AND granted AND mode='ShareRowExclusiveLock'")
        .bind(pid).fetch_one(&*pool).await.unwrap();
    assert_eq!(mode, "ShareRowExclusiveLock");
    tokio::time::timeout(
        Timeout::from_secs(3),
        sqlx::query("INSERT INTO user_sessions(session_id,user_id) VALUES('foreign-key','fk')")
            .execute(&mut *writer),
    )
    .await
    .expect("a writer holding the source table must complete its FK lookup")
    .unwrap();
    writer.rollback().await.unwrap();
    tokio::time::timeout(Timeout::from_secs(10), privacy)
        .await
        .unwrap()
        .unwrap();
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn uninitialized_retention_records_a_monotonic_cutoff_before_the_first_rebuild() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    let cutoff = Utc::now() - Duration::days(90);
    let mut tx = pool.begin().await.unwrap();
    let initialized: bool = sqlx::query_scalar("SELECT public.prepare_reporting_privacy()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(!initialized);
    sqlx::query("INSERT INTO users(id,name,email) VALUES('old','old','old@example.test')")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_sessions(session_id,user_id,started_at,last_activity_at,ended_at,expires_at,ip_address) VALUES('old-session','old','2020-01-01','2020-01-01','2020-01-02','2020-01-02','192.0.2.42')")
        .execute(&mut *tx).await.unwrap();
    let expired: i64 = sqlx::query_scalar("SELECT public.expire_reporting_sessions($1)")
        .bind(cutoff)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(expired, 1);
    sqlx::query("SELECT public.finish_reporting_privacy($1)")
        .bind(cutoff)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let state: (bool, i64, i64) = sqlx::query_as("SELECT initialized,generation,(SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting') FROM analytics_projection_state WHERE singleton")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(
        state,
        (false, 0, 0),
        "privacy must consume its own changes without claiming a complete baseline"
    );
    initialize_and_drain(&db, 0).await;
    let count: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM user_sessions) + (SELECT count(*) FROM analytics_report_user_sessions)")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(count, 0);
    let mut backwards = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *backwards)
        .await
        .unwrap();
    let error = sqlx::query("SELECT public.finish_reporting_privacy($1)")
        .bind(cutoff - Duration::days(1))
        .execute(&mut *backwards)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("22023")
    );
    backwards.rollback().await.unwrap();
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn children_arriving_before_parent_projections_survive_and_orphans_do_not_rebuild() {
    use systemprompt_analytics::projection::{ReportingProjector, ReportingRow};
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users(id,name,email) VALUES('parent','parent','parent@example.test')")
        .execute(&*pool)
        .await
        .unwrap();
    initialize_and_drain(&db, 1).await;
    sqlx::query(
        "INSERT INTO user_contexts(context_id,user_id,name) VALUES('ctx','parent','context')",
    )
    .execute(&*pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agent_tasks(task_id,context_id,user_id) VALUES('task','ctx','parent')",
    )
    .execute(&*pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO task_messages(id,task_id,message_id,role,context_id,sequence_number) VALUES(42,'task','message','user','ctx',1)")
        .execute(&*pool).await.unwrap();
    sqlx::query("INSERT INTO ai_requests(id,request_id,user_id,context_id,provider,model,actor_kind,actor_id,cost_microdollars,status) VALUES('request','request','parent','ctx','test','model','job','test',253,'failed')")
        .execute(&*pool).await.unwrap();
    sqlx::query("INSERT INTO ai_request_messages(id,request_id,role,content,sequence_number) VALUES('request-message','request','user','case',1)")
        .execute(&*pool).await.unwrap();
    let children: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT fact->'data' FROM event_outbox WHERE fact->'data'->>'source' = 'task_messages'",
    )
    .fetch_all(&*pool)
    .await
    .unwrap();
    assert_eq!(children.len(), 1);
    let message_facts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_outbox WHERE fact->'data'->>'source' = 'ai_request_messages'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(
        message_facts, 0,
        "stored messages are counted, not projected"
    );
    let mut tx = pool.begin().await.unwrap();
    for value in children {
        let fact: ReportingRow = serde_json::from_value(value).unwrap();
        assert!(
            ReportingProjector::apply_fact(&mut tx, &fact)
                .await
                .unwrap()
        );
    }
    let parents: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM analytics_report_agent_tasks) + (SELECT count(*) FROM analytics_report_ai_requests)")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(
        parents, 0,
        "children were applied before either projected parent existed"
    );
    tx.commit().await.unwrap();
    // context, task, request insert, the request's message_count update, and
    // the task message applied above but still pending in the outbox
    assert_eq!(reporting::process_pending(&db, 20).await.unwrap(), 5);
    let children: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_report_task_messages")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(children, 1);
    let request: (i64, i32) = sqlx::query_as(
        "SELECT cost_microdollars, message_count FROM analytics_report_ai_requests WHERE id='request'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(request, (253, 1));
    sqlx::query("DELETE FROM agent_tasks WHERE task_id='task'")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM ai_requests WHERE id='request'")
        .execute(&*pool)
        .await
        .unwrap();
    reporting::process_pending(&db, 20).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT public.finish_reporting_privacy(NULL)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    reporting::rebuild(&db).await.unwrap();
    let children: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_report_task_messages")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(children, 0);
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn correction_outside_retention_removes_previous_row_and_fences_older_replay() {
    use systemprompt_analytics::projection::{ReportingProjector, ReportingRow};
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users(id,name,email) VALUES('correction','correction','correction@example.test')")
        .execute(&*pool).await.unwrap();
    initialize_and_drain(&db, 1).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT public.finish_reporting_privacy($1)")
        .bind(Utc::now() - Duration::days(90))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO ai_requests(id,request_id,user_id,context_id,provider,model,actor_kind,actor_id,cost_microdollars) VALUES('corrected','corrected','correction','ctx','test','model','job','test',253)")
        .execute(&*pool).await.unwrap();
    let original: serde_json::Value = sqlx::query_scalar(
        "SELECT fact->'data' FROM event_outbox WHERE fact->'data'->>'key'='corrected'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    let original: ReportingRow = serde_json::from_value(original).unwrap();
    assert_eq!(reporting::process_pending(&db, 10).await.unwrap(), 1);
    let present: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM analytics_report_ai_requests WHERE id='corrected'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(present, 1);
    sqlx::query("UPDATE ai_requests SET created_at='2020-01-01' WHERE id='corrected'")
        .execute(&*pool)
        .await
        .unwrap();
    let correction: i64 = sqlx::query_scalar("SELECT (fact->'data'->>'revision')::bigint FROM event_outbox WHERE fact->'data'->>'key'='corrected' AND processed_at IS NULL")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(reporting::process_pending(&db, 10).await.unwrap(), 1);
    let mut tx = pool.begin().await.unwrap();
    let absent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM analytics_report_ai_requests WHERE id='corrected'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        absent, 0,
        "an accepted correction must remove the previous contribution immediately"
    );
    let accepted_revision: i64 = sqlx::query_scalar("SELECT revision FROM analytics_projection_revisions WHERE source='ai_requests' AND entity_key='corrected'")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(accepted_revision, correction);
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &original)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.prepare_reporting_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT public.finish_reporting_privacy(NULL)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let residues: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM analytics_report_ai_requests) + (SELECT count(*) FROM analytics_projection_revisions) + (SELECT count(*) FROM event_outbox WHERE consumer='analytics_reporting')")
        .fetch_one(&*pool).await.unwrap();
    assert_eq!(residues, 0);
    let mut tx = pool.begin().await.unwrap();
    assert!(
        !ReportingProjector::apply_fact(&mut tx, &original)
            .await
            .unwrap()
    );
    tx.rollback().await.unwrap();
    cleanup(&admin, &pool, &database).await;
}

#[path = "reporting_privacy_initialization.rs"]
mod initialization;
