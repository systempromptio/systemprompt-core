//! DB-backed tests for [`RetentionScheduler::start`].

use systemprompt_logging::services::retention::{RetentionConfig, RetentionScheduler};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn scheduler_disabled_short_circuits_ok() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let mut config = RetentionConfig::default();
    config.enabled = false;
    let s = RetentionScheduler::new(config, db);
    s.start().await.expect("disabled scheduler returns Ok");
}

#[tokio::test]
async fn scheduler_enabled_starts_cron_job() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "0 0 0 * * *".to_owned();
    let s = RetentionScheduler::new(config, db);
    s.start().await.expect("enabled scheduler installs job");
}

#[tokio::test]
async fn scheduler_rejects_invalid_cron_schedule() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "not a cron expression".to_owned();
    let s = RetentionScheduler::new(config, db);
    s.start()
        .await
        .expect_err("invalid schedule must fail job creation");
}

#[tokio::test]
async fn scheduled_cleanup_deletes_logs_older_than_retention() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let raw = db.pool_arc().unwrap().as_ref().clone();

    let log_id = format!("retention-{}", uuid::Uuid::new_v4().simple());
    let stale_timestamp = chrono::Utc::now() - chrono::Duration::days(365);
    sqlx::query!(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id, session_id, trace_id)
         VALUES ($1, $2, 'INFO', 'retention_test', 'stale row', 'ret-user', 'ret-session', $3)",
        log_id.as_str(),
        stale_timestamp,
        log_id.as_str()
    )
    .execute(&raw)
    .await
    .expect("insert stale log");

    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "* * * * * *".to_owned();
    RetentionScheduler::new(config, db)
        .start()
        .await
        .expect("scheduler starts");

    let mut remaining = 1_i64;
    for _ in 0..100 {
        remaining = sqlx::query_scalar!("SELECT COUNT(*) FROM logs WHERE id = $1", log_id.as_str())
            .fetch_one(&raw)
            .await
            .unwrap()
            .unwrap_or(0);
        if remaining == 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(
        remaining, 0,
        "cron-fired retention cleanup must delete the year-old row"
    );
}
