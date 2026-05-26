//! DB-backed tests for [`RetentionScheduler::start`].

use systemprompt_logging::services::retention::{RetentionConfig, RetentionScheduler};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn scheduler_disabled_short_circuits_ok() {
    let Ok(url) = fixture_database_url() else { return };
    let Ok(db) = fixture_db_pool(&url).await else { return };
    let mut config = RetentionConfig::default();
    config.enabled = false;
    let s = RetentionScheduler::new(config, db);
    s.start().await.expect("disabled scheduler returns Ok");
}

#[tokio::test]
async fn scheduler_enabled_starts_cron_job() {
    let Ok(url) = fixture_database_url() else { return };
    let Ok(db) = fixture_db_pool(&url).await else { return };
    let mut config = RetentionConfig::default();
    config.enabled = true;
    config.schedule = "0 0 0 * * *".to_owned();
    let s = RetentionScheduler::new(config, db);
    s.start().await.expect("enabled scheduler installs job");
}
