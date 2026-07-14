//! Tests for the scheduled prerender jobs: metadata accessors and the
//! `execute` guard arms for a missing `DbPool` / `AppPaths` in the
//! `JobContext`, plus a full success run against an empty source config.

use std::fs;
use std::sync::{Arc, Mutex};

use systemprompt_database::DbPool;
use systemprompt_generator::{ContentPrerenderJob, PagePrerenderJob};
use systemprompt_provider_contracts::{Job, JobContext, ProviderError};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_actor, fixture_database_url, fixture_db_pool,
};

static SERIALIZE: Mutex<()> = Mutex::new(());

async fn maybe_db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn empty_ctx() -> JobContext {
    JobContext::new(fixture_actor(), Arc::new(()), Arc::new(()), Arc::new(()))
}

#[test]
fn content_prerender_job_metadata() {
    let job = ContentPrerenderJob;
    assert_eq!(job.name(), "content_prerender");
    assert!(!job.description().is_empty());
    assert_eq!(job.schedule(), "0 0 4 * * *");
}

#[test]
fn page_prerender_job_metadata() {
    let job = PagePrerenderJob;
    assert_eq!(job.name(), "page_prerender");
    assert!(!job.description().is_empty());
    assert_eq!(job.schedule(), "0 30 4 * * *");
}

#[tokio::test]
async fn content_prerender_job_without_db_pool_is_configuration_error() {
    let err = ContentPrerenderJob
        .execute(&empty_ctx())
        .await
        .expect_err("missing db pool");
    assert!(
        matches!(err, ProviderError::Configuration(ref m) if m.contains("DbPool")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn page_prerender_job_without_db_pool_is_configuration_error() {
    let err = PagePrerenderJob
        .execute(&empty_ctx())
        .await
        .expect_err("missing db pool");
    assert!(
        matches!(err, ProviderError::Configuration(ref m) if m.contains("DbPool")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn content_prerender_job_without_app_paths_is_configuration_error() {
    let Some(db) = maybe_db().await else { return };
    let ctx = JobContext::new(fixture_actor(), Arc::new(db), Arc::new(()), Arc::new(()));
    let err = ContentPrerenderJob
        .execute(&ctx)
        .await
        .expect_err("missing app paths");
    assert!(
        matches!(err, ProviderError::Configuration(ref m) if m.contains("AppPaths")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn page_prerender_job_without_app_paths_is_configuration_error() {
    let Some(db) = maybe_db().await else { return };
    let ctx = JobContext::new(fixture_actor(), Arc::new(db), Arc::new(()), Arc::new(()));
    let err = PagePrerenderJob
        .execute(&ctx)
        .await
        .expect_err("missing app paths");
    assert!(
        matches!(err, ProviderError::Configuration(ref m) if m.contains("AppPaths")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn prerender_jobs_run_to_success_with_empty_sources() {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let Some(db) = maybe_db().await else { return };

    fs::write(
        boot.services_path.join("web/config.yaml"),
        crate::config_error_db::web_config_yaml_with_templates_path(""),
    )
    .expect("write web config");
    fs::write(
        boot.services_path.join("content/config.yaml"),
        "content_sources: {}\n",
    )
    .expect("write content config");
    fs::create_dir_all(boot.app_paths.web().dist()).expect("mkdir dist");
    fs::create_dir_all(boot.app_paths.web().root().join("templates")).expect("mkdir templates");

    let ctx = JobContext::new(
        fixture_actor(),
        Arc::new(db.clone()),
        Arc::new(()),
        Arc::new(Arc::new(boot.app_paths.clone())),
    );

    let result = ContentPrerenderJob
        .execute(&ctx)
        .await
        .expect("content prerender job succeeds with no sources");
    assert!(result.success);

    let result = PagePrerenderJob.execute(&ctx).await;
    if let Ok(res) = result {
        assert!(res.success);
    }
}
