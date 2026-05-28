//! Drives the public job-execution surface
//! (`execute_copy_extension_assets`) end-to-end against an empty
//! extension registry so the no-assets-to-copy branch is covered. With
//! `inventory`-registered extensions absent in this test binary the
//! registry yields zero required assets and the job succeeds with empty
//! stats.

use std::sync::Arc;

use systemprompt_generator::{
    ContentPrerenderJob, PagePrerenderJob, execute_copy_extension_assets,
};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_provider_contracts::{Job, JobContext};
use tempfile::TempDir;

fn paths_in(tmp: &TempDir) -> AppPaths {
    let root = tmp.path();
    let services = root.join("services");
    let bin = root.join("bin");
    std::fs::create_dir_all(&services).expect("mkdir services");
    std::fs::create_dir_all(&bin).expect("mkdir bin");
    std::fs::create_dir_all(root.join("web/dist")).expect("mkdir web/dist");

    let paths = PathsConfig {
        system: root.to_string_lossy().to_string(),
        services: services.to_string_lossy().to_string(),
        bin: bin.to_string_lossy().to_string(),
        web_path: Some(root.join("web").to_string_lossy().to_string()),
        storage: Some(root.join("storage").to_string_lossy().to_string()),
        geoip_database: None,
    };
    std::fs::create_dir_all(root.join("storage/files")).expect("mkdir storage/files");

    AppPaths::from_profile(&paths).expect("from_profile")
}

#[tokio::test]
async fn copy_extension_assets_no_op_when_registry_is_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let paths = paths_in(&tmp);

    let result = execute_copy_extension_assets(&paths)
        .await
        .expect("copy job must succeed when no assets are registered");

    assert!(
        result.success,
        "JobResult must report success; got {result:?}"
    );
}

fn empty_job_ctx() -> JobContext {
    JobContext::new(
        Actor::system(UserId::new("test-user")),
        Arc::new(()),
        Arc::new(()),
        Arc::new(()),
    )
}

#[test]
fn content_prerender_job_metadata() {
    let job = ContentPrerenderJob;
    assert_eq!(job.name(), "content_prerender");
    assert!(job.description().to_lowercase().contains("content"));
    assert_eq!(job.schedule(), "0 0 4 * * *");
    assert!(job.enabled());
    assert!(job.tags().is_empty());
}

#[test]
fn page_prerender_job_metadata() {
    let job = PagePrerenderJob;
    assert_eq!(job.name(), "page_prerender");
    assert!(job.description().to_lowercase().contains("page"));
    assert_eq!(job.schedule(), "0 30 4 * * *");
    assert!(job.enabled());
}

#[tokio::test]
async fn content_prerender_job_errors_when_db_pool_missing() {
    let job = ContentPrerenderJob;
    let ctx = empty_job_ctx();
    let err = job
        .execute(&ctx)
        .await
        .expect_err("must fail with no DbPool");
    assert!(err.to_string().to_lowercase().contains("dbpool"));
}

#[tokio::test]
async fn page_prerender_job_errors_when_db_pool_missing() {
    let job = PagePrerenderJob;
    let ctx = empty_job_ctx();
    let err = job
        .execute(&ctx)
        .await
        .expect_err("must fail with no DbPool");
    assert!(err.to_string().to_lowercase().contains("dbpool"));
}
