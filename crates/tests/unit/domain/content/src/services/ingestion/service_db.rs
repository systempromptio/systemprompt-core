//! DB-backed behavioral tests for [`IngestionService::ingest_directory`].
//!
//! These drive the full ingest pipeline against a real Postgres pool: markdown
//! scanning (extension/subdirectory filtering), content-record construction,
//! and the create/update/skip/unchanged reconciliation arms.

use systemprompt_content::models::IngestionSource;
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::{IngestionOptions, IngestionService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, LocaleCode, SourceId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct Ctx {
    pool: DbPool,
    dir: tempfile::TempDir,
    source_id: SourceId,
    category: CategoryId,
    source_name: String,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(Ctx {
        pool,
        dir: tempfile::tempdir().expect("tempdir"),
        source_id: SourceId::new(format!("ing-src-{}", Uuid::new_v4())),
        category: CategoryId::new("docs"),
        source_name: "docs".to_owned(),
    })
}

impl Ctx {
    fn source(&self) -> IngestionSource<'_> {
        IngestionSource::new(&self.source_id, &self.source_name, &self.category)
    }

    async fn cleanup(&self) {
        let repo = ContentRepository::new(&self.pool).expect("repo");
        repo.delete_by_source(&self.source_id)
            .await
            .expect("cleanup");
    }
}

fn write_md(dir: &std::path::Path, slug: &str, title: &str, body: &str) {
    let md = format!(
        "---\ntitle: \"{title}\"\nslug: \"{slug}\"\nauthor: \"Author\"\npublished_at: \"2024-01-15\"\nkind: \"article\"\ndescription: \"d\"\n---\n\n{body}\n"
    );
    std::fs::write(dir.join(format!("{slug}.md")), md).expect("write md");
}

#[tokio::test]
async fn ingest_creates_updates_skips_and_detects_unchanged() {
    let Some(ctx) = setup().await else { return };
    let slug = format!("life-{}", Uuid::new_v4().simple());
    write_md(ctx.dir.path(), &slug, "First", "Original body");

    // First pass creates the row.
    let report = ctx
        .service_ingest(IngestionOptions::default().with_override(true))
        .await;
    assert_eq!(report.files_processed, 1);
    assert!(report.errors.is_empty(), "unexpected errors: {report:?}");

    // Re-ingesting the identical file is a no-op (hash matches).
    let report = ctx
        .service_ingest(IngestionOptions::default().with_override(true))
        .await;
    assert_eq!(report.unchanged_count, 1);

    // Change the body; without override the update is skipped.
    write_md(ctx.dir.path(), &slug, "First", "Rewritten body");
    let report = ctx.service_ingest(IngestionOptions::default()).await;
    assert_eq!(report.skipped_count, 1, "{report:?}");

    // With override the row is updated.
    let report = ctx
        .service_ingest(IngestionOptions::default().with_override(true))
        .await;
    assert_eq!(report.unchanged_count, 0);
    assert_eq!(report.skipped_count, 0);

    let repo = ContentRepository::new(&ctx.pool).expect("repo");
    let stored = repo
        .get_by_source_and_slug(&ctx.source_id, &slug, &LocaleCode::new("en"))
        .await
        .expect("query")
        .expect("row");
    assert_eq!(stored.body, "Rewritten body");

    ctx.cleanup().await;
}

#[tokio::test]
async fn dry_run_reports_would_create_then_would_update() {
    let Some(ctx) = setup().await else { return };
    let slug = format!("dry-{}", Uuid::new_v4().simple());
    write_md(ctx.dir.path(), &slug, "Dry", "Body one");

    let report = ctx
        .service_ingest(IngestionOptions::default().with_dry_run(true))
        .await;
    assert_eq!(report.would_create, vec![slug.clone()]);

    // Really create it, then a dry-run over a changed file reports would_update.
    ctx.service_ingest(IngestionOptions::default().with_override(true))
        .await;
    write_md(ctx.dir.path(), &slug, "Dry", "Body two");
    let report = ctx
        .service_ingest(
            IngestionOptions::default()
                .with_override(true)
                .with_dry_run(true),
        )
        .await;
    assert_eq!(report.would_update, vec![slug.clone()]);

    ctx.cleanup().await;
}

#[tokio::test]
async fn invalid_published_date_is_reported_as_a_per_file_error() {
    let Some(ctx) = setup().await else { return };
    let slug = format!("baddate-{}", Uuid::new_v4().simple());
    // Passes the YYYY-MM-DD format gate but is not a real calendar date, so it
    // survives scanning and fails in content construction.
    let md = format!(
        "---\ntitle: \"Bad\"\nslug: \"{slug}\"\nauthor: \"A\"\npublished_at: \"2024-13-40\"\nkind: \"article\"\ndescription: \"d\"\n---\n\nbody\n"
    );
    std::fs::write(ctx.dir.path().join(format!("{slug}.md")), md).expect("write");

    let report = ctx
        .service_ingest(IngestionOptions::default().with_override(true))
        .await;
    assert_eq!(report.files_processed, 0);
    assert_eq!(report.errors.len(), 1, "{report:?}");
    assert!(
        report.errors[0].contains("published_at") || report.errors[0].contains(&slug),
        "error should reference the offending file: {:?}",
        report.errors
    );

    ctx.cleanup().await;
}

#[tokio::test]
async fn links_frontmatter_is_persisted_on_the_content_row() {
    let Some(ctx) = setup().await else { return };
    let slug = format!("links-{}", Uuid::new_v4().simple());
    let md = format!(
        "---\ntitle: \"Linked\"\nslug: \"{slug}\"\nauthor: \"A\"\npublished_at: \"2024-01-15\"\nkind: \"article\"\ndescription: \"d\"\nlinks:\n  - title: \"Home\"\n    url: \"https://example.com\"\n---\n\nbody\n"
    );
    std::fs::write(ctx.dir.path().join(format!("{slug}.md")), md).expect("write");

    let report = ctx
        .service_ingest(IngestionOptions::default().with_override(true))
        .await;
    assert_eq!(report.files_processed, 1, "{report:?}");

    let repo = ContentRepository::new(&ctx.pool).expect("repo");
    let stored = repo
        .get_by_source_and_slug(&ctx.source_id, &slug, &LocaleCode::new("en"))
        .await
        .expect("query")
        .expect("row");
    let links = stored.links.to_string();
    assert!(links.contains("https://example.com"), "links: {links}");

    ctx.cleanup().await;
}

#[tokio::test]
async fn scanner_skips_non_markdown_and_nested_dirs_and_warns() {
    let Some(ctx) = setup().await else { return };
    // A non-markdown file and an extensionless file are ignored.
    std::fs::write(ctx.dir.path().join("notes.txt"), "ignore me").expect("write txt");
    std::fs::write(ctx.dir.path().join("LICENSE"), "ignore me").expect("write noext");
    // A nested directory holding markdown is skipped in non-recursive mode and
    // triggers the "consider --recursive" warning because the root has no md.
    let nested = ctx.dir.path().join("nested");
    std::fs::create_dir_all(&nested).expect("nested");
    write_md(&nested, "buried", "Buried", "deep");

    let report = ctx.service_ingest(IngestionOptions::default()).await;
    assert_eq!(report.files_found, 0, "root has no markdown: {report:?}");
    assert!(
        report.warnings.iter().any(|w| w.contains("--recursive")),
        "expected a recursive-scan hint: {:?}",
        report.warnings
    );

    // Recursive mode reaches the nested file.
    let report = ctx
        .service_ingest(
            IngestionOptions::default()
                .with_recursive(true)
                .with_override(true),
        )
        .await;
    assert_eq!(report.files_processed, 1, "{report:?}");

    ctx.cleanup().await;
}

impl Ctx {
    async fn service_ingest(
        &self,
        options: IngestionOptions,
    ) -> systemprompt_content::IngestionReport {
        let service = IngestionService::new(&self.pool).expect("service");
        service
            .ingest_directory(self.dir.path(), &self.source(), options)
            .await
            .expect("ingest")
    }
}
