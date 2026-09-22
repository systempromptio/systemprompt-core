use super::fixture;
use sqlx::PgPool;
use systemprompt_runtime::reporting::{self, RebuildOutcome};

async fn cleanup(admin: &PgPool, pool: &PgPool, database: &str) {
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {database} WITH (FORCE)"
    )))
    .execute(admin)
    .await
    .unwrap();
}

async fn report_count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn stale_rebuild_marker_is_taken_over_and_fresh_one_is_left_alone() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ('u', 'baseline', 'u@example.test')")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE analytics_projection_state SET generation = 3, initialized = FALSE,
         rebuild_started_at = NOW() - interval '10 minutes',
         rebuild_heartbeat_at = NOW() - interval '10 minutes', rebuild_source = 'logs'",
    )
    .execute(&*pool)
    .await
    .unwrap();
    assert_eq!(
        reporting::initialize(&db).await.unwrap(),
        RebuildOutcome::Rebuilt
    );
    let status = reporting::status(&db).await.unwrap();
    assert!(status.initialized);
    assert_eq!(status.generation, 4);
    assert!(status.rebuild_started_at.is_none() && status.rebuild_source.is_none());
    assert_eq!(report_count(&pool, "analytics_report_users").await, 1);

    sqlx::query(
        "UPDATE analytics_projection_state SET initialized = FALSE,
         rebuild_started_at = NOW(), rebuild_heartbeat_at = NOW(),
         rebuild_source = 'ai_requests', rebuild_rows = 42",
    )
    .execute(&*pool)
    .await
    .unwrap();
    assert_eq!(
        reporting::initialize(&db).await.unwrap(),
        RebuildOutcome::InProgressElsewhere
    );
    let status = reporting::status(&db).await.unwrap();
    assert!(!status.initialized);
    assert_eq!(status.generation, 4);
    assert_eq!(status.rebuild_source.as_deref(), Some("ai_requests"));
    assert_eq!(status.rebuild_rows, 42);

    reporting::rebuild(&db).await.unwrap();
    let status = reporting::status(&db).await.unwrap();
    assert!(status.initialized);
    assert_eq!(status.generation, 5);
    cleanup(&admin, &pool, &database).await;
}

#[tokio::test]
async fn user_deleted_before_the_baseline_is_not_resurrected_and_later_facts_apply() {
    let (admin, db, database) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ('kept', 'kept', 'kept@example.test'), ('gone', 'gone', 'gone@example.test')")
        .execute(&*pool).await.unwrap();
    sqlx::query("INSERT INTO user_sessions (session_id, user_id, landing_page, request_count, is_ai_crawler) VALUES ('kept-session', 'kept', '/', 1, false), ('gone-session', 'gone', '/', 1, false)")
        .execute(&*pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT public.begin_user_privacy()")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_sessions WHERE user_id = 'gone'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = 'gone'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(!reporting::status(&db).await.unwrap().initialized);

    assert_eq!(
        reporting::initialize(&db).await.unwrap(),
        RebuildOutcome::Rebuilt
    );
    while reporting::process_pending(&db, 100).await.unwrap() > 0 {}
    assert_eq!(reporting::status(&db).await.unwrap().pending_count, 0);
    let users: Vec<String> =
        sqlx::query_scalar("SELECT id FROM analytics_report_users ORDER BY id")
            .fetch_all(&*pool)
            .await
            .unwrap();
    assert_eq!(users, vec!["kept".to_owned()]);
    let sessions: Vec<String> =
        sqlx::query_scalar("SELECT session_id FROM analytics_report_user_sessions ORDER BY 1")
            .fetch_all(&*pool)
            .await
            .unwrap();
    assert_eq!(sessions, vec!["kept-session".to_owned()]);

    sqlx::query("UPDATE users SET name = 'renamed' WHERE id = 'kept'")
        .execute(&*pool)
        .await
        .unwrap();
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 1);
    let name: String =
        sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id = 'kept'")
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert_eq!(name, "renamed");
    cleanup(&admin, &pool, &database).await;
}
