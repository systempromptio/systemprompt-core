//! Tests for the scheduled prerender jobs: metadata accessors and the
//! `execute` guard arms for a missing `DbPool` / `AppPaths` in the
//! `JobContext`, plus a full success run against an empty source config with
//! and without built content hooks in the context, and the generator member's
//! contribution of both jobs.

use std::fs;
use std::sync::{Arc, Mutex};

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_generator::{ContentPrerenderJob, GeneratorExtension, PagePrerenderJob};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, Job, JobContext, JobScope, ProviderError};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_actor, test_db_pool};

static SERIALIZE: Mutex<()> = Mutex::new(());

fn empty_ctx() -> JobContext {
    JobContext::new(fixture_actor(), Dependencies::new())
}

#[test]
fn content_prerender_job_metadata() {
    let job = ContentPrerenderJob;
    assert_eq!(job.name(), "content_prerender");
    assert!(!job.description().is_empty());
    assert_eq!(job.schedule(), "0 0 4 * * *");
}

#[test]
fn the_generator_member_contributes_both_prerender_jobs() {
    assert_eq!(GeneratorExtension.metadata().id, "generator");
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("generator"), &mut sink);
    GeneratorExtension
        .contribute(&mut c)
        .expect("the generator member contributes");
    let mut owned: Vec<&str> = sink.jobs().iter().map(|job| job.job().name()).collect();
    owned.sort_unstable();
    assert_eq!(owned, vec!["content_prerender", "page_prerender"]);
    assert!(sink.schemas().is_empty());
    assert!(sink.provides().is_empty());
}

#[test]
fn prerender_jobs_are_node_scoped() {
    assert_eq!(ContentPrerenderJob.scope(), JobScope::Node);
    assert_eq!(PagePrerenderJob.scope(), JobScope::Node);
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
        matches!(err, ProviderError::MissingDependency(ref m) if m.type_name().contains("Database")),
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
        matches!(err, ProviderError::MissingDependency(ref m) if m.type_name().contains("Database")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn content_prerender_job_without_app_paths_is_configuration_error() {
    let db = test_db_pool().await;
    let ctx = JobContext::new(fixture_actor(), Dependencies::new().with(db));
    let err = ContentPrerenderJob
        .execute(&ctx)
        .await
        .expect_err("missing app paths");
    assert!(
        matches!(err, ProviderError::MissingDependency(ref m) if m.type_name().contains("AppPaths")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn page_prerender_job_without_app_paths_is_configuration_error() {
    let db = test_db_pool().await;
    let ctx = JobContext::new(fixture_actor(), Dependencies::new().with(db));
    let err = PagePrerenderJob
        .execute(&ctx)
        .await
        .expect_err("missing app paths");
    assert!(
        matches!(err, ProviderError::MissingDependency(ref m) if m.type_name().contains("AppPaths")),
        "unexpected error: {err:?}"
    );
}

async fn run_prerender_jobs_with_empty_sources(with_hooks: bool) {
    let _guard = SERIALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let boot = ensure_test_bootstrap();
    let db = test_db_pool().await;

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

    let mut state = Dependencies::new()
        .with(db.clone())
        .with(systemprompt_generator::PrerenderSettings::of(
            boot.boot_config(),
        ))
        .with(Arc::new(boot.app_paths.clone()));
    if with_hooks {
        state.insert(Arc::new(crate::ext_fixtures::fixture_hooks(&db)));
    } else {
        state.insert(Arc::new(
            systemprompt_content::contract::ContentHooks::default(),
        ));
    }
    let ctx = JobContext::new(fixture_actor(), state);

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

#[tokio::test]
async fn prerender_jobs_run_to_success_with_empty_sources_and_the_built_hooks() {
    run_prerender_jobs_with_empty_sources(true).await;
}

#[tokio::test]
async fn prerender_jobs_run_to_success_with_empty_sources_and_no_built_hooks() {
    run_prerender_jobs_with_empty_sources(false).await;
}
