//! DB-backed tests for `IngestionService`. These wire scanner + frontmatter
//! parser + `ContentRepository` together end-to-end against a real Postgres,
//! exercising create / update / unchanged / skipped paths and the dry-run
//! preview output.

use std::fs;
use std::path::Path;
use systemprompt_content::models::{IngestionOptions, IngestionSource};
use systemprompt_content::services::IngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, SourceId};
use tempfile::TempDir;

async fn try_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn write_markdown(dir: &Path, name: &str, body: &str) {
    let path = dir.join(format!("{name}.md"));
    fs::write(&path, body).expect("write fixture markdown");
}

fn sample_frontmatter(slug: &str, title: &str) -> String {
    format!(
        "---\ntitle: \"{title}\"\nslug: \"{slug}\"\nkind: article\nauthor: \"Integration Test\"\npublished_at: \"2025-01-01\"\n---\n\n# {title}\n\nSample body.\n",
    )
}

#[tokio::test]
async fn ingestion_service_new_succeeds() {
    let Some(db) = try_db().await else {
        return;
    };
    assert!(IngestionService::new(&db).is_ok());
}

#[tokio::test]
async fn ingest_directory_dry_run_lists_would_create_for_new_files() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = IngestionService::new(&db).expect("service");
    let dir = TempDir::new().expect("tempdir");
    let slug = format!("dry-{}", uuid::Uuid::new_v4().simple());
    write_markdown(dir.path(), &slug, &sample_frontmatter(&slug, "Dry Run"));

    let source_id = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let category_id = CategoryId::new("articles".to_owned());
    let source = IngestionSource::new(&source_id, "test-src", &category_id);

    let report = svc
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default()
                .with_recursive(false)
                .with_dry_run(true),
        )
        .await
        .expect("ingest dry-run");

    assert_eq!(report.files_found, 1);
    assert_eq!(report.files_processed, 1);
    assert!(
        report.would_create.iter().any(|s| s == &slug),
        "would_create should contain {slug}, got {:?}",
        report.would_create
    );

    let repo = systemprompt_content::repository::ContentRepository::new(&db).expect("repo");
    repo.delete_by_source(&source_id).await.ok();
}

#[tokio::test]
async fn ingest_directory_creates_then_unchanged_on_second_pass() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = IngestionService::new(&db).expect("service");
    let dir = TempDir::new().expect("tempdir");
    let slug = format!("ing-{}", uuid::Uuid::new_v4().simple());
    write_markdown(dir.path(), &slug, &sample_frontmatter(&slug, "Ingest"));

    let source_id = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let category_id = CategoryId::new("articles".to_owned());
    let source = IngestionSource::new(&source_id, "test-src", &category_id);

    let first = svc
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default().with_recursive(false),
        )
        .await
        .expect("first ingest");
    assert_eq!(first.files_processed, 1);
    assert!(first.errors.is_empty(), "errors: {:?}", first.errors);
    assert_eq!(first.unchanged_count, 0);

    let second = svc
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default().with_recursive(false),
        )
        .await
        .expect("second ingest");
    assert_eq!(second.unchanged_count, 1, "second pass should be unchanged");

    let repo = systemprompt_content::repository::ContentRepository::new(&db).expect("repo");
    repo.delete_by_source(&source_id).await.ok();
}

#[tokio::test]
async fn ingest_directory_skips_modified_when_override_disabled() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = IngestionService::new(&db).expect("service");
    let dir = TempDir::new().expect("tempdir");
    let slug = format!("skip-{}", uuid::Uuid::new_v4().simple());
    write_markdown(dir.path(), &slug, &sample_frontmatter(&slug, "Original"));

    let source_id = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let category_id = CategoryId::new("articles".to_owned());
    let source = IngestionSource::new(&source_id, "test-src", &category_id);

    svc.ingest_directory(
        dir.path(),
        &source,
        IngestionOptions::default().with_recursive(false),
    )
    .await
    .expect("first ingest");

    write_markdown(dir.path(), &slug, &sample_frontmatter(&slug, "Updated"));

    let second = svc
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default()
                .with_recursive(false)
                .with_override(false),
        )
        .await
        .expect("second ingest");
    assert_eq!(
        second.skipped_count, 1,
        "modified file with override=false must be skipped, report={second:?}"
    );

    let repo = systemprompt_content::repository::ContentRepository::new(&db).expect("repo");
    repo.delete_by_source(&source_id).await.ok();
}

#[tokio::test]
async fn ingest_directory_reports_parse_errors() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = IngestionService::new(&db).expect("service");
    let dir = TempDir::new().expect("tempdir");
    fs::write(dir.path().join("broken.md"), "no frontmatter here").expect("write");

    let source_id = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let category_id = CategoryId::new("articles".to_owned());
    let source = IngestionSource::new(&source_id, "test-src", &category_id);

    let report = svc
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default().with_recursive(false),
        )
        .await
        .expect("ingest broken");
    assert!(
        !report.errors.is_empty() || report.files_processed == 0,
        "broken markdown should surface as an error or be skipped, got {report:?}"
    );
}
