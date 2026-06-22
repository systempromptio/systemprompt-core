//! DB-backed tests for [`ContentLocalSync`].
//!
//! Drives `calculate_diff`, `sync_to_disk`, and `sync_to_db` against seeded
//! `markdown_content` rows and temp directories, asserting files written,
//! orphan handling, and result counters. Tests early-return when
//! `DATABASE_URL` is unset; each cleans up its seeded source.

use std::fs;
use std::path::PathBuf;

use systemprompt_content::models::CreateContentParams;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_sync::{
    ContentDiffEntry, ContentDiffItem, ContentDiffResult, ContentLocalSync, DiffStatus,
    LocalSyncDirection, compute_content_hash,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use tempfile::TempDir;
use uuid::Uuid;

const KIND: &str = "article";

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        ensure_test_bootstrap();
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        pool
    }};
}

fn unique_source() -> SourceId {
    SourceId::new(format!("local-src-{}", Uuid::new_v4()))
}

// `markdown_content` enforces UNIQUE(slug, locale) globally; namespace slugs.
fn slug(base: &str) -> String {
    format!("{base}-{}", Uuid::new_v4())
}

async fn seed(repo: &ContentRepository, source: &SourceId, slug: &str, title: &str, body: &str) {
    let params = CreateContentParams::new(
        slug.to_owned(),
        title.to_owned(),
        "desc".to_owned(),
        body.to_owned(),
        source.clone(),
    )
    .with_kind(KIND.to_owned())
    .with_version_hash(compute_content_hash(body, title));
    repo.create(&params).await.expect("seed content");
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    let repo = ContentRepository::new(pool).expect("repo");
    repo.delete_by_source(source).await.expect("cleanup");
}

fn item(slug: &str, source: &SourceId, status: DiffStatus) -> ContentDiffItem {
    ContentDiffItem {
        slug: slug.to_owned(),
        source_id: source.clone(),
        status,
        disk_hash: None,
        db_hash: None,
        disk_updated_at: None,
        db_updated_at: None,
        title: Some(slug.to_owned()),
    }
}

fn entry(
    name: &str,
    source: &SourceId,
    path: PathBuf,
    diff: ContentDiffResult,
) -> ContentDiffEntry {
    ContentDiffEntry {
        name: name.to_owned(),
        source_id: source.clone(),
        category_id: CategoryId::new("guides"),
        path,
        allowed_content_types: vec![KIND.to_owned()],
        diff,
    }
}

#[tokio::test]
async fn sync_to_disk_exports_modified_and_removed() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let mod_slug = slug("mod-post");
    let rm_slug = slug("rm-post");
    seed(&repo, &source, &mod_slug, "Mod Post", "body one").await;
    seed(&repo, &source, &rm_slug, "Rm Post", "body two").await;

    let dir = TempDir::new().expect("tempdir");
    let diff = ContentDiffResult {
        source_id: source.clone(),
        modified: vec![item(&mod_slug, &source, DiffStatus::Modified)],
        removed: vec![item(&rm_slug, &source, DiffStatus::Removed)],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_disk(&entries, false).await.expect("sync");

    assert_eq!(result.direction, LocalSyncDirection::ToDisk);
    assert_eq!(result.items_synced, 2);
    assert!(result.errors.is_empty());
    assert!(dir.path().join(format!("{mod_slug}.md")).exists());
    assert!(dir.path().join(format!("{rm_slug}.md")).exists());
    let body = fs::read_to_string(dir.path().join(format!("{mod_slug}.md"))).expect("read");
    assert!(body.contains("body one"));

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_disk_records_error_for_missing_db_row() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");

    // No DB rows seeded, so the modified slug is absent in the database.
    let diff = ContentDiffResult {
        source_id: source.clone(),
        modified: vec![item("ghost", &source, DiffStatus::Modified)],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_disk(&entries, false).await.expect("sync");

    assert_eq!(result.items_synced, 0);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("ghost"));

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_disk_skips_added_when_not_deleting_orphans() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");
    let diff = ContentDiffResult {
        source_id: source.clone(),
        added: vec![
            item("a1", &source, DiffStatus::Added),
            item("a2", &source, DiffStatus::Added),
        ],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_disk(&entries, false).await.expect("sync");

    assert_eq!(result.items_skipped, 2);
    assert_eq!(result.items_deleted, 0);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_disk_deletes_orphan_files() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");
    // Orphan file on disk that has no DB counterpart.
    fs::write(dir.path().join("orphan.md"), "stale").expect("write");

    let diff = ContentDiffResult {
        source_id: source.clone(),
        added: vec![item("orphan", &source, DiffStatus::Added)],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_disk(&entries, true).await.expect("sync");

    assert_eq!(result.items_deleted, 1);
    assert!(!dir.path().join("orphan.md").exists());

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_disk_deletes_orphan_blog_directory() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");
    let post_dir = dir.path().join("orphan-blog");
    fs::create_dir_all(&post_dir).expect("mkdir");
    fs::write(post_dir.join("index.md"), "stale").expect("write");

    let diff = ContentDiffResult {
        source_id: source.clone(),
        added: vec![item("orphan-blog", &source, DiffStatus::Added)],
        ..Default::default()
    };
    let entries = vec![entry("blog", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_disk(&entries, true).await.expect("sync");

    assert_eq!(result.items_deleted, 1);
    assert!(!post_dir.exists());

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn calculate_diff_delegates_to_calculator() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let present = slug("present");
    seed(&repo, &source, &present, "Present", "body").await;

    let sync = ContentLocalSync::new(pool.clone());
    let empty = TempDir::new().expect("tempdir");
    let diff = sync
        .calculate_diff(&source, empty.path(), &[KIND.to_owned()])
        .await
        .expect("calculate_diff");

    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.removed[0].slug, present);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_db_skips_removed_without_orphan_deletion() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");

    // Empty ingestion directory: no files processed. A removed entry is
    // skipped because delete_orphans is false.
    let diff = ContentDiffResult {
        source_id: source.clone(),
        removed: vec![item("old", &source, DiffStatus::Removed)],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_db(&entries, false, false).await.expect("sync");

    assert_eq!(result.direction, LocalSyncDirection::ToDatabase);
    assert_eq!(result.items_synced, 0);
    assert_eq!(result.items_skipped, 1);
    assert_eq!(result.items_deleted, 0);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn sync_to_db_deletes_removed_orphans() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");

    let diff = ContentDiffResult {
        source_id: source.clone(),
        removed: vec![item("old", &source, DiffStatus::Removed)],
        ..Default::default()
    };
    let entries = vec![entry("guides", &source, dir.path().to_path_buf(), diff)];

    let sync = ContentLocalSync::new(pool.clone());
    let result = sync.sync_to_db(&entries, true, false).await.expect("sync");

    assert_eq!(result.items_deleted, 1);
    assert_eq!(result.items_skipped, 0);

    cleanup(&pool, &source).await;
}
