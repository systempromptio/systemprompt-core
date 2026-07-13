//! `FileIngestionJob::execute` branches: full scan/insert/skip cycle against
//! the bootstrap storage tree, missing pool, missing global config, missing
//! images directory, and repository failures via closed pools.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_files::{FileIngestionJob, FileRepository, FilesConfig};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::{
    TestBootstrap, closed_db_pool, ensure_test_bootstrap, fixture_db_pool,
};
use systemprompt_traits::{Job, JobContext};

fn job_ctx(pool_any: Arc<dyn std::any::Any + Send + Sync>) -> JobContext {
    let actor = Actor::job(UserId::new("files-job-test"), "test".to_owned());
    JobContext::new(actor, pool_any, Arc::new(()), Arc::new(()))
}

fn pool_ctx(pool: &DbPool) -> JobContext {
    job_ctx(Arc::new(Arc::clone(pool)))
}

async fn live_pool(bootstrap: &TestBootstrap) -> Option<DbPool> {
    fixture_db_pool(&bootstrap.database_url).await.ok()
}

#[tokio::test]
async fn execute_ingests_images_then_skips_on_rerun() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = FilesConfig::get().expect("bootstrap initialised FilesConfig");
    let generated = cfg.generated_images();
    std::fs::create_dir_all(&generated).expect("mkdir generated");

    let extensions = ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"];
    for ext in extensions {
        std::fs::write(b.storage_path.join(format!("pic.{ext}")), b"img").expect("write image");
    }
    std::fs::write(generated.join("ai.png"), b"generated").expect("write generated image");
    std::fs::write(b.storage_path.join("notes.txt"), b"text").expect("write txt");
    std::fs::write(b.storage_path.join("noext"), b"raw").expect("write extensionless");

    let job = FileIngestionJob::new();
    let ctx = pool_ctx(&pool);
    let result = job.execute(&ctx).await.expect("execute");
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("Found: 8, Inserted: 8, Skipped: 0, Errors: 0")
    );
    assert_eq!(result.items_processed, Some(8));
    assert_eq!(result.items_failed, Some(0));

    let repo = FileRepository::new(&pool).expect("repo");
    let expected_mimes = [
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
        ("ico", "image/x-icon"),
    ];
    for (ext, mime) in expected_mimes {
        let path = b.storage_path.join(format!("pic.{ext}"));
        let row = repo
            .find_by_path(&path.to_string_lossy())
            .await
            .expect("find")
            .expect("row present");
        assert_eq!(row.mime_type, mime);
        assert!(!row.ai_content);
        assert_eq!(row.size_bytes, Some(3));
        assert_eq!(row.public_url, cfg.public_url(&format!("pic.{ext}")));
    }

    let generated_row = repo
        .find_by_path(&generated.join("ai.png").to_string_lossy())
        .await
        .expect("find generated")
        .expect("generated row present");
    assert!(
        generated_row.ai_content,
        "files under the generated dir are flagged as AI content"
    );

    assert!(
        repo.find_by_path(&b.storage_path.join("notes.txt").to_string_lossy())
            .await
            .expect("find txt")
            .is_none(),
        "non-image extensions are not ingested"
    );

    let rerun = job.execute(&ctx).await.expect("re-execute");
    assert_eq!(
        rerun.message.as_deref(),
        Some("Found: 8, Inserted: 0, Skipped: 8, Errors: 0")
    );
}

#[tokio::test]
async fn execute_without_db_pool_is_configuration_error() {
    ensure_test_bootstrap();
    let job = FileIngestionJob::new();
    let ctx = job_ctx(Arc::new(()));

    let err = job.execute(&ctx).await.expect_err("no pool");
    let message = err.to_string();
    assert!(
        message.contains("Database pool not available in job context"),
        "unexpected error: {message}"
    );
}

#[tokio::test]
async fn execute_without_files_config_is_configuration_error() {
    // No bootstrap: FilesConfig::get() must fail in this process.
    let Ok(url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
    else {
        return;
    };
    let Ok(read) = sqlx::PgPool::connect(&url).await else {
        return;
    };
    let pool: DbPool = Arc::new(Database::from_pools(Arc::new(read), None));

    let job = FileIngestionJob::new();
    let ctx = pool_ctx(&pool);
    let err = job.execute(&ctx).await.expect_err("config missing");
    let message = err.to_string();
    assert!(
        message.contains("FilesConfig::init() not called"),
        "unexpected error: {message}"
    );
}

#[tokio::test]
async fn execute_with_missing_images_dir_short_circuits() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    std::fs::remove_dir_all(&b.storage_path).expect("remove storage root");

    let job = FileIngestionJob::new();
    let result = job.execute(&pool_ctx(&pool)).await.expect("execute");
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("Images directory not found")
    );
}

#[tokio::test]
async fn execute_counts_existence_check_failures() {
    let b = ensure_test_bootstrap();
    if live_pool(b).await.is_none() {
        return;
    }
    std::fs::write(b.storage_path.join("broken.png"), b"img").expect("write image");

    let job = FileIngestionJob::new();
    let closed = closed_db_pool().await;
    let result = job.execute(&pool_ctx(&closed)).await.expect("execute");
    assert_eq!(
        result.message.as_deref(),
        Some("Found: 1, Inserted: 0, Skipped: 0, Errors: 1")
    );
}

#[tokio::test]
async fn execute_counts_insert_failures() {
    let b = ensure_test_bootstrap();
    if live_pool(b).await.is_none() {
        return;
    }
    std::fs::write(b.storage_path.join("unsaved.png"), b"img").expect("write image");

    // Live read pool (existence check passes), closed write pool (insert
    // fails), so the error arm of insert_file_record is exercised.
    let read = sqlx::PgPool::connect(&b.database_url)
        .await
        .expect("read pool");
    let closed = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed")
        .expect("lazy pool");
    closed.close().await;
    let pool: DbPool = Arc::new(Database::from_pools(Arc::new(read), Some(Arc::new(closed))));

    let job = FileIngestionJob::new();
    let result = job.execute(&pool_ctx(&pool)).await.expect("execute");
    assert_eq!(
        result.message.as_deref(),
        Some("Found: 1, Inserted: 0, Skipped: 0, Errors: 1")
    );
}
