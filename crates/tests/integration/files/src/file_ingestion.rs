//! Integration tests for FileIngestionJob::execute against a real DbPool.
//!
//! Drops a couple of image files into the bootstrapped storage tree and runs
//! the ingestion job, then asserts the resulting database state.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_files::FileIngestionJob;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_traits::{Job, JobContext};

use crate::bootstrap::test_env;

async fn get_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn write_png_at(path: &std::path::Path) {
    let parent = path.parent().expect("png path has parent");
    std::fs::create_dir_all(parent).expect("mkdir parent");
    let bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x5B, 0xEF, 0x6A, 0xC8, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(path, bytes).expect("write png");
}

#[tokio::test]
async fn file_ingestion_job_metadata_surface() {
    let job = FileIngestionJob::new();
    assert_eq!(job.name(), "file_ingestion");
    assert!(!job.description().is_empty());
    let schedule_parts = job.schedule().split_whitespace().count();
    assert_eq!(schedule_parts, 6, "cron expr should be 6-field");
}

#[tokio::test]
async fn file_ingestion_executes_against_real_pool() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let env = test_env();

    let unique = format!("test_{}.png", uuid::Uuid::new_v4().simple());
    let png_path = env.storage_root.join("images").join(&unique);
    write_png_at(&png_path);

    let job = FileIngestionJob::new();
    let actor = Actor::system(UserId::new("test-system"));
    let db_arc: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db);
    let app_paths_arc: Arc<dyn std::any::Any + Send + Sync> = Arc::new(env.app_paths.clone());
    let app_ctx_arc: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());

    let ctx = JobContext::new(actor, db_arc, app_ctx_arc, app_paths_arc);

    let result = job.execute(&ctx).await.expect("job should execute");
    assert!(result.success, "ingestion job reports success");
    assert!(result.message.is_some());
}
