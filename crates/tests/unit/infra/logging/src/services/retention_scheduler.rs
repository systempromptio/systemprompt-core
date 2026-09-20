//! DB-backed tests for [`RetentionScheduler::start`].

use systemprompt_logging::services::retention::{RetentionConfig, RetentionScheduler};
use systemprompt_test_fixtures::{DisposableDb, fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn scheduler_disabled_short_circuits_ok() {
    let url = fixture_database_url().expect("logging database fixture must be configured");
    let db = fixture_db_pool(&url)
        .await
        .expect("logging database fixture must connect");
    let mut config = RetentionConfig::default();
    config.enabled = false;
    let s = RetentionScheduler::new(config, &db).expect("retention scheduler");
    s.start().await.expect("disabled scheduler returns Ok");
}

#[tokio::test]
async fn scheduler_enabled_starts_cron_job() {
    let url = fixture_database_url().expect("logging database fixture must be configured");
    let db = fixture_db_pool(&url)
        .await
        .expect("logging database fixture must connect");
    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "0 0 0 * * *".to_owned();
    let s = RetentionScheduler::new(config, &db).expect("retention scheduler");
    s.start().await.expect("enabled scheduler installs job");
}

#[tokio::test]
async fn scheduler_rejects_invalid_cron_schedule() {
    let url = fixture_database_url().expect("logging database fixture must be configured");
    let db = fixture_db_pool(&url)
        .await
        .expect("logging database fixture must connect");
    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "not a cron expression".to_owned();
    let s = RetentionScheduler::new(config, &db).expect("retention scheduler");
    s.start()
        .await
        .expect_err("invalid schedule must fail job creation");
}

// The cron tick has to make progress on its own task; under an instrumented
// (coverage) build a current-thread runtime starves it long enough that the
// job body never fires, which silently voids this test's whole point.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_cleanup_deletes_only_logs_older_than_retention() {
    let database = DisposableDb::installed("logging_retention_scheduler")
        .await
        .expect("isolated installed logging database");
    let db = database.pool().await.expect("isolated logging pool");
    let raw = db.pool_arc().expect("raw logging pool").as_ref().clone();

    let marker = format!("retention-{}", uuid::Uuid::new_v4().simple());
    let stale_id = format!("{marker}-stale");
    let fresh_id = format!("{marker}-fresh");
    let stale_timestamp = chrono::Utc::now() - chrono::Duration::days(365);
    for (id, timestamp, message) in [
        (&stale_id, stale_timestamp, "stale row"),
        (&fresh_id, chrono::Utc::now(), "fresh row"),
    ] {
        sqlx::query(
            "INSERT INTO logs (id, timestamp, level, module, message, user_id, session_id, trace_id)
             VALUES ($1, $2, 'INFO', 'retention_test', $3, 'ret-user', 'ret-session', $4)",
        )
        .bind(id.as_str())
        .bind(timestamp)
        .bind(message)
        .bind(id.as_str())
        .execute(&raw)
        .await
        .expect("insert retention fixture log");
    }

    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "* * * * * *".to_owned();
    RetentionScheduler::new(config, &db)
        .expect("retention scheduler")
        .start()
        .await
        .expect("scheduler starts");

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let stale_remaining =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE id = $1")
                .bind(stale_id.as_str())
                .fetch_one(&raw)
                .await
                .expect("read stale fixture count");
        if stale_remaining == 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "this scheduler did not execute its retention job before the deadline"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let fresh_remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE id = $1")
        .bind(fresh_id.as_str())
        .fetch_one(&raw)
        .await
        .expect("read fresh fixture count");
    assert_eq!(
        fresh_remaining, 1,
        "retention must preserve a log inside every configured cutoff"
    );

    drop(raw);
    drop(db);
    database.drop_now().await;
}
