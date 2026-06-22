//! DB-backed tests for [`ContentDiffCalculator`].
//!
//! Seeds `markdown_content` rows for a unique source, writes matching /
//! mismatching markdown files to a temp directory, and asserts the structured
//! diff (`added`/`modified`/`removed`/`unchanged`) the calculator produces.
//! Each test cleans up its seeded rows. Tests early-return when `DATABASE_URL`
//! is unset.

use std::fs;
use std::path::Path;

use systemprompt_content::models::CreateContentParams;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_sync::{ContentDiffCalculator, compute_content_hash};
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
    SourceId::new(format!("diff-src-{}", Uuid::new_v4()))
}

// Content rows have a global UNIQUE(slug, locale) constraint, so concurrent
// tests must not share slug literals. Namespace every slug per test run.
fn slug(base: &str) -> String {
    format!("{base}-{}", Uuid::new_v4())
}

async fn seed(
    repo: &ContentRepository,
    source: &SourceId,
    slug: &str,
    title: &str,
    body: &str,
    version_hash: &str,
) {
    let params = CreateContentParams::new(
        slug.to_owned(),
        title.to_owned(),
        "desc".to_owned(),
        body.to_owned(),
        source.clone(),
    )
    .with_kind(KIND.to_owned())
    .with_version_hash(version_hash.to_owned());
    repo.create(&params).await.expect("seed content");
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    let repo = ContentRepository::new(pool).expect("repo");
    repo.delete_by_source(source).await.expect("cleanup");
}

fn write_md(dir: &Path, slug: &str, title: &str, body: &str) {
    let contents = format!("---\nkind: {KIND}\nslug: {slug}\ntitle: {title}\n---\n\n{body}\n");
    fs::write(dir.join(format!("{slug}.md")), contents).expect("write md");
}

#[tokio::test]
async fn missing_disk_path_yields_only_removed() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let hash = compute_content_hash("body", "Title");
    let s = slug("only-in-db");
    seed(&repo, &source, &s, "Title", "body", &hash).await;

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let nonexistent = Path::new("/tmp/does-not-exist-sync-diff-xyz");
    let diff = calc
        .calculate_diff(&source, nonexistent, &[KIND.to_owned()])
        .await
        .expect("diff");

    assert!(diff.added.is_empty());
    assert!(diff.modified.is_empty());
    assert_eq!(diff.unchanged, 0);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.removed[0].slug, s);
    assert_eq!(diff.removed[0].db_hash.as_deref(), Some(hash.as_str()));
    assert_eq!(diff.source_id, source);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn disk_only_file_is_added() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");
    write_md(dir.path(), "new-post", "New Post", "fresh body");

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &[KIND.to_owned()])
        .await
        .expect("diff");

    assert_eq!(diff.added.len(), 1, "disk-only file should be Added");
    let item = &diff.added[0];
    assert_eq!(item.slug, "new-post");
    assert_eq!(item.title.as_deref(), Some("New Post"));
    assert_eq!(
        item.disk_hash.as_deref(),
        Some(compute_content_hash("fresh body", "New Post").as_str())
    );
    assert!(item.db_hash.is_none());
    assert!(diff.modified.is_empty());
    assert!(diff.removed.is_empty());
    assert_eq!(diff.unchanged, 0);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn matching_hash_is_unchanged() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let body = "stable body";
    let title = "Stable";
    let hash = compute_content_hash(body, title);
    let s = slug("stable");
    seed(&repo, &source, &s, title, body, &hash).await;

    let dir = TempDir::new().expect("tempdir");
    write_md(dir.path(), &s, title, body);

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &[KIND.to_owned()])
        .await
        .expect("diff");

    assert_eq!(diff.unchanged, 1);
    assert!(diff.added.is_empty());
    assert!(diff.modified.is_empty());
    assert!(diff.removed.is_empty());

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn differing_hash_is_modified() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let title = "Drifted";
    let db_hash = compute_content_hash("old body", title);
    let s = slug("drifted");
    seed(&repo, &source, &s, title, "old body", &db_hash).await;

    let dir = TempDir::new().expect("tempdir");
    write_md(dir.path(), &s, title, "new body");

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &[KIND.to_owned()])
        .await
        .expect("diff");

    assert_eq!(diff.modified.len(), 1);
    let item = &diff.modified[0];
    assert_eq!(item.slug, s);
    assert_eq!(item.db_hash.as_deref(), Some(db_hash.as_str()));
    assert_eq!(
        item.disk_hash.as_deref(),
        Some(compute_content_hash("new body", title).as_str())
    );
    assert!(item.db_updated_at.is_some());
    assert_eq!(diff.unchanged, 0);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn disallowed_kind_is_ignored() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");
    // File declares kind=article, but the allowed set excludes it.
    write_md(dir.path(), "wrong-kind", "Wrong Kind", "body");

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &["skill".to_owned()])
        .await
        .expect("diff");

    assert!(diff.added.is_empty(), "non-allowed kind must be skipped");
    assert!(diff.modified.is_empty());
    assert!(diff.removed.is_empty());
    assert_eq!(diff.unchanged, 0);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn non_md_and_invalid_frontmatter_files_skipped() {
    let pool = pool_or_skip!();
    let source = unique_source();
    let dir = TempDir::new().expect("tempdir");

    // A valid markdown file plus noise: a non-md file and an md file missing
    // frontmatter. Only the valid file should surface as Added.
    write_md(dir.path(), "valid", "Valid", "body");
    fs::write(dir.path().join("notes.txt"), "ignored").expect("txt");
    fs::write(dir.path().join("broken.md"), "no frontmatter here").expect("broken");

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &[KIND.to_owned()])
        .await
        .expect("diff");

    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.added[0].slug, "valid");

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn combined_added_modified_removed_unchanged() {
    let pool = pool_or_skip!();
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source();
    let locale = LocaleCode::new("en");

    let stable = slug("stable");
    let drift = slug("drift");
    let gone = slug("gone");
    let brand_new = slug("brand-new");
    let stable_hash = compute_content_hash("same", "Stable");
    seed(&repo, &source, &stable, "Stable", "same", &stable_hash).await;
    seed(
        &repo,
        &source,
        &drift,
        "Drift",
        "db-body",
        &compute_content_hash("db-body", "Drift"),
    )
    .await;
    seed(
        &repo,
        &source,
        &gone,
        "Gone",
        "db-body",
        &compute_content_hash("db-body", "Gone"),
    )
    .await;

    let dir = TempDir::new().expect("tempdir");
    write_md(dir.path(), &stable, "Stable", "same");
    write_md(dir.path(), &drift, "Drift", "disk-body");
    write_md(dir.path(), &brand_new, "Brand New", "new");

    let calc = ContentDiffCalculator::new(&pool).expect("calc");
    let diff = calc
        .calculate_diff(&source, dir.path(), &[KIND.to_owned()])
        .await
        .expect("diff");

    assert_eq!(diff.unchanged, 1);
    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.added[0].slug, brand_new);
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.modified[0].slug, drift);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.removed[0].slug, gone);

    // Sanity: DB still has the rows we expect for this source.
    let rows = repo
        .list_by_source(&source, &locale)
        .await
        .expect("list_by_source");
    assert_eq!(rows.len(), 3);

    cleanup(&pool, &source).await;
}
